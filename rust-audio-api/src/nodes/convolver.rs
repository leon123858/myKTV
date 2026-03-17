use crate::types::{AUDIO_UNIT_SIZE, AudioUnit};
use crossbeam_channel::{Sender, bounded};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use thread_priority::*;

pub struct ConvolverConfig {
    pub stereo: bool,
    pub growth_exponent: u32,
}

impl Default for ConvolverConfig {
    fn default() -> Self {
        Self {
            stereo: true,
            growth_exponent: 2,
        }
    }
}

pub struct AtomicF32(AtomicU32);

impl AtomicF32 {
    #[inline(always)]
    pub fn new(v: f32) -> Self {
        Self(AtomicU32::new(v.to_bits()))
    }

    #[inline(always)]
    pub fn fetch_add(&self, val: f32, order: Ordering) {
        let mut current = self.0.load(order);
        loop {
            let current_f32 = f32::from_bits(current);
            let new_f32 = current_f32 + val;
            let new_bits = new_f32.to_bits();
            match self
                .0
                .compare_exchange_weak(current, new_bits, order, order)
            {
                Ok(_) => break,
                Err(c) => current = c,
            }
        }
    }

    #[inline(always)]
    pub fn swap(&self, val: f32, order: Ordering) -> f32 {
        let old = self.0.swap(val.to_bits(), order);
        f32::from_bits(old)
    }
}

struct PartitionBlock {
    size: usize,
    offset: usize,
    time_domain_l: Vec<f32>,
    time_domain_r: Vec<f32>,
    fft_data_l: Arc<[rustfft::num_complex::Complex<f32>]>,
    fft_data_r: Arc<[rustfft::num_complex::Complex<f32>]>,
}

#[derive(Clone)]
struct TaskMsg {
    carry_read_ptr: usize,
    input_l: [f32; AUDIO_UNIT_SIZE],
    input_r: [f32; AUDIO_UNIT_SIZE],
}

pub struct ConvolverNode {
    stereo: bool,
    block_0_l: [f32; AUDIO_UNIT_SIZE],
    block_0_r: [f32; AUDIO_UNIT_SIZE],
    task_senders: Vec<Sender<TaskMsg>>,
    carry_buffer_l: Arc<Vec<AtomicF32>>,
    carry_buffer_r: Arc<Vec<AtomicF32>>,
    carry_mask: usize,
    carry_read_ptr: usize,
    shared_read_ptr: Arc<AtomicUsize>,
    drop_count: Arc<AtomicUsize>,
    catch_up_count: Arc<AtomicUsize>,
}

impl Drop for ConvolverNode {
    fn drop(&mut self) {
        // Drop task_senders, which closes channels, terminating worker threads
        self.task_senders.clear();
    }
}

impl ConvolverNode {
    pub fn from_file(
        path: &str,
        target_sample_rate: u32,
        max_len: Option<usize>,
    ) -> anyhow::Result<Self> {
        Self::from_file_with_config(
            path,
            target_sample_rate,
            max_len,
            ConvolverConfig::default(),
        )
    }

    pub fn from_file_with_config(
        path: &str,
        target_sample_rate: u32,
        max_len: Option<usize>,
        config: ConvolverConfig,
    ) -> anyhow::Result<Self> {
        let mut reader = hound::WavReader::open(path)?;
        let spec = reader.spec();
        let mut ir = Vec::new();

        if spec.sample_format == hound::SampleFormat::Float {
            let mut iter = reader.samples::<f32>();
            while let Some(Ok(l)) = iter.next() {
                let r = if spec.channels == 2 {
                    iter.next().unwrap().unwrap_or(l)
                } else {
                    l
                };
                ir.push([l, r]);
            }
        } else {
            panic!("unexpected ir file format")
        }

        let mut ir = if spec.sample_rate != target_sample_rate {
            Self::resample_ir(&ir, spec.sample_rate, target_sample_rate)
        } else {
            ir
        };

        if let Some(max) = max_len {
            if ir.len() > max {
                ir.truncate(max);
            }
        }

        Ok(Self::with_config(&ir, config))
    }

    fn resample_ir(ir: &[[f32; 2]], from_hz: u32, to_hz: u32) -> Vec<[f32; 2]> {
        use dasp::signal::Signal;
        let signal = dasp::signal::from_iter(ir.iter().cloned());
        let ring_buffer = dasp::ring_buffer::Fixed::from([[0.0; 2]; AUDIO_UNIT_SIZE]);
        let sinc = dasp::interpolate::sinc::Sinc::new(ring_buffer);
        let mut converter = signal.from_hz_to_hz(sinc, from_hz as f64, to_hz as f64);

        let new_len = (ir.len() as f64 * (to_hz as f64 / from_hz as f64)).ceil() as usize;
        let mut new_ir = Vec::with_capacity(new_len);
        for _ in 0..new_len {
            new_ir.push(converter.next());
        }
        new_ir
    }

    pub fn new(ir: &[[f32; 2]]) -> Self {
        Self::with_config(ir, ConvolverConfig::default())
    }

    pub fn with_config(ir: &[[f32; 2]], config: ConvolverConfig) -> Self {
        let stereo = config.stereo;
        let blocks = Self::partition_ir(ir, config.growth_exponent);

        let max_block_size = blocks.last().map(|b| b.size).unwrap_or(AUDIO_UNIT_SIZE);
        let mut capacity = (ir.len() + max_block_size * 2).next_power_of_two() * 4;
        if capacity < 65536 {
            capacity = 65536;
        }
        let carry_mask = capacity - 1;

        let carry_buffer_l = Arc::new(
            (0..capacity)
                .map(|_| AtomicF32::new(0.0))
                .collect::<Vec<_>>(),
        );
        let carry_buffer_r = Arc::new(
            (0..capacity)
                .map(|_| AtomicF32::new(0.0))
                .collect::<Vec<_>>(),
        );

        let drop_count = Arc::new(AtomicUsize::new(0));
        let shared_read_ptr = Arc::new(AtomicUsize::new(0));
        let catch_up_count = Arc::new(AtomicUsize::new(0));

        let mut b0_l = [0.0f32; AUDIO_UNIT_SIZE];
        let mut b0_r = [0.0f32; AUDIO_UNIT_SIZE];
        if !blocks.is_empty() {
            b0_l.copy_from_slice(&blocks[0].time_domain_l[..AUDIO_UNIT_SIZE]);
            b0_r.copy_from_slice(&blocks[0].time_domain_r[..AUDIO_UNIT_SIZE]);
        }

        let mut task_senders = Vec::new();
        let max_queue_len = 2048;

        let mut planner = realfft::RealFftPlanner::<f32>::new();
        let mut fft_plans = HashMap::new();
        for block in &blocks {
            if block.size == AUDIO_UNIT_SIZE && block.offset == 0 {
                continue;
            }
            let len2 = block.size * 2;
            if !fft_plans.contains_key(&len2) {
                fft_plans.insert(
                    len2,
                    (
                        planner.plan_fft_forward(len2),
                        planner.plan_fft_inverse(len2),
                    ),
                );
            }
        }

        for block in blocks.into_iter().skip(1) {
            let (tx, rx) = bounded::<TaskMsg>(max_queue_len);
            task_senders.push(tx);

            let s = block.size;
            let hist_cap = s.max(AUDIO_UNIT_SIZE).next_power_of_two();
            let hist_mask = hist_cap - 1;

            let len2 = s * 2;
            let out_len = s + 1;
            let (fft, ifft) = fft_plans.get(&len2).unwrap();
            let fft_plan = Arc::clone(fft);
            let ifft_plan = Arc::clone(ifft);

            let block_fft_l = block.fft_data_l;
            let block_fft_r = block.fft_data_r;
            let block_offset = block.offset;

            let worker_carry_l = Arc::clone(&carry_buffer_l);
            let worker_carry_r = Arc::clone(&carry_buffer_r);
            let worker_drop_count = Arc::clone(&drop_count);
            let worker_shared_read_ptr = Arc::clone(&shared_read_ptr);
            let worker_catch_up_count = Arc::clone(&catch_up_count);
            let worker_stereo = stereo;

            std::thread::spawn(move || {
                if let Err(e) = set_current_thread_priority(ThreadPriority::Max) {
                    eprintln!("警告: 無法提升卷積區塊執行緒優先權: {:?}", e);
                }

                let mut head = 0;
                let mut history_l = vec![0.0f32; hist_cap];
                let mut history_r = vec![0.0f32; hist_cap];

                let mut pad_l = vec![0.0f32; len2];
                let mut pad_r = vec![0.0f32; len2];
                let mut out_l_slice = vec![rustfft::num_complex::Complex::new(0.0, 0.0); out_len];
                let mut out_r_slice = vec![rustfft::num_complex::Complex::new(0.0, 0.0); out_len];
                let mut res_l = vec![0.0f32; len2];
                let mut res_r = vec![0.0f32; len2];

                while let Ok(task) = rx.recv() {
                    let queue_len = rx.len();

                    // 動態背壓：隊列越滿，允許的年齡越低 (越容易丟棄)
                    let max_queue_age = if queue_len > max_queue_len / 2 { 2 } else { 8 };

                    let mut dropped = false;
                    if queue_len > max_queue_age {
                        dropped = true;
                        worker_drop_count.fetch_add(1, Ordering::Relaxed);
                    }

                    for i in 0..AUDIO_UNIT_SIZE {
                        history_l[(head + i) & hist_mask] = task.input_l[i];
                        if worker_stereo {
                            history_r[(head + i) & hist_mask] = task.input_r[i];
                        }
                    }
                    head = (head + AUDIO_UNIT_SIZE) & hist_mask;

                    if (task.carry_read_ptr + AUDIO_UNIT_SIZE) % s == 0 {
                        if dropped {
                            continue;
                        }

                        let start_idx = (head + hist_cap - s) & hist_mask;

                        for i in 0..s {
                            pad_l[i] = history_l[(start_idx + i) & hist_mask];
                        }
                        pad_l[s..].fill(0.0);

                        if worker_stereo {
                            for i in 0..s {
                                pad_r[i] = history_r[(start_idx + i) & hist_mask];
                            }
                            pad_r[s..].fill(0.0);
                        }

                        fft_plan.process(&mut pad_l, &mut out_l_slice).unwrap();
                        if worker_stereo {
                            fft_plan.process(&mut pad_r, &mut out_r_slice).unwrap();
                        }

                        for i in 0..out_len {
                            out_l_slice[i] = out_l_slice[i] * block_fft_l[i];
                            if worker_stereo {
                                out_r_slice[i] = out_r_slice[i] * block_fft_r[i];
                            }
                        }

                        ifft_plan.process(&mut out_l_slice, &mut res_l).unwrap();
                        let scale = 1.0 / (len2 as f32);
                        for x in res_l.iter_mut() {
                            *x *= scale;
                        }

                        if worker_stereo {
                            ifft_plan.process(&mut out_r_slice, &mut res_r).unwrap();
                            for x in res_r.iter_mut() {
                                *x *= scale;
                            }
                        }

                        let current_ptr = worker_shared_read_ptr.load(Ordering::Relaxed);
                        let task_ptr = task.carry_read_ptr;
                        let capacity = carry_mask + 1;

                        let current_real = if current_ptr < task_ptr {
                            current_ptr + capacity
                        } else {
                            current_ptr
                        };

                        let out_base_real = task_ptr + AUDIO_UNIT_SIZE + block_offset;
                        let safe_current_real = current_real + AUDIO_UNIT_SIZE;

                        let skip = if out_base_real < safe_current_real {
                            worker_catch_up_count.fetch_add(1, Ordering::Relaxed);
                            safe_current_real - out_base_real
                        } else {
                            0
                        };

                        for i in skip..(len2 - 1) {
                            let idx = (out_base_real + i) & carry_mask;
                            worker_carry_l[idx].fetch_add(res_l[i], Ordering::Relaxed);
                            if worker_stereo {
                                worker_carry_r[idx].fetch_add(res_r[i], Ordering::Relaxed);
                            }
                        }
                    }
                }
            });
        }

        Self {
            stereo,
            block_0_l: b0_l,
            block_0_r: b0_r,
            task_senders,
            carry_buffer_l,
            carry_buffer_r,
            carry_mask,
            carry_read_ptr: 0,
            shared_read_ptr,
            drop_count,
            catch_up_count,
        }
    }

    fn partition_ir(ir: &[[f32; 2]], growth_exponent: u32) -> Vec<PartitionBlock> {
        let mut blocks = Vec::new();
        let mut offset = 0;
        let growth_factor = growth_exponent.max(1) as usize;

        let b0_len = AUDIO_UNIT_SIZE;
        let b0_l = Self::take_slice_padded(ir, offset, b0_len, 0);
        let b0_r = Self::take_slice_padded(ir, offset, b0_len, 1);
        blocks.push(PartitionBlock {
            size: b0_len,
            offset,
            time_domain_l: b0_l,
            time_domain_r: b0_r,
            fft_data_l: Arc::new([]),
            fft_data_r: Arc::new([]),
        });
        offset += b0_len;

        let mut current_size = AUDIO_UNIT_SIZE * (growth_exponent as usize);
        let mut planner = realfft::RealFftPlanner::<f32>::new();

        while offset < ir.len() {
            let len = current_size;
            let l_slice = Self::take_slice_padded(ir, offset, len, 0);
            let r_slice = Self::take_slice_padded(ir, offset, len, 1);

            let fft = planner.plan_fft_forward(len * 2);

            let mut padded_l = vec![0.0; len * 2];
            padded_l[..len].copy_from_slice(&l_slice);
            let mut out_l = fft.make_output_vec();
            fft.process(&mut padded_l, &mut out_l).unwrap();

            let mut padded_r = vec![0.0; len * 2];
            padded_r[..len].copy_from_slice(&r_slice);
            let mut out_r = fft.make_output_vec();
            fft.process(&mut padded_r, &mut out_r).unwrap();

            blocks.push(PartitionBlock {
                size: len,
                offset,
                time_domain_l: vec![],
                time_domain_r: vec![],
                fft_data_l: out_l.into(),
                fft_data_r: out_r.into(),
            });

            // Even if taking short blocks at the end, continue until offset covers IR.
            offset += len;
            current_size *= growth_factor;
        }
        blocks
    }

    fn take_slice_padded(ir: &[[f32; 2]], offset: usize, len: usize, ch: usize) -> Vec<f32> {
        let mut res = vec![0.0; len];
        if offset < ir.len() {
            let take = (ir.len() - offset).min(len);
            for i in 0..take {
                res[i] = ir[offset + i][ch];
            }
        }
        res
    }

    #[inline(always)]
    pub fn process(&mut self, input: Option<&AudioUnit>, output: &mut AudioUnit) {
        let empty_input = crate::types::empty_audio_unit();
        let input_ref = input.unwrap_or(&empty_input);

        let mut in_l = [0.0f32; AUDIO_UNIT_SIZE];
        let mut in_r = [0.0f32; AUDIO_UNIT_SIZE];
        for i in 0..AUDIO_UNIT_SIZE {
            in_l[i] = input_ref[i][0];
            in_r[i] = input_ref[i][1];
        }

        let mask = self.carry_mask;

        let mut b0_out_l = [0.0f32; 127];
        let mut b0_out_r = [0.0f32; 127];
        for i in 0..AUDIO_UNIT_SIZE {
            for j in 0..AUDIO_UNIT_SIZE {
                b0_out_l[i + j] += in_l[i] * self.block_0_l[j];
                if self.stereo {
                    b0_out_r[i + j] += in_r[i] * self.block_0_r[j];
                }
            }
        }

        for i in 0..127 {
            let idx = (self.carry_read_ptr + i) & mask;
            self.carry_buffer_l[idx].fetch_add(b0_out_l[i], Ordering::Relaxed);
            if self.stereo {
                self.carry_buffer_r[idx].fetch_add(b0_out_r[i], Ordering::Relaxed);
            }
        }

        for i in 0..AUDIO_UNIT_SIZE {
            let idx = (self.carry_read_ptr + i) & mask;
            let out_l = self.carry_buffer_l[idx].swap(0.0, Ordering::Relaxed);
            let out_r = self.carry_buffer_r[idx].swap(0.0, Ordering::Relaxed);

            output[i][0] = out_l;
            if self.stereo {
                output[i][1] = out_r;
            } else {
                output[i][1] = out_l;
            }
        }

        let task = TaskMsg {
            carry_read_ptr: self.carry_read_ptr,
            input_l: in_l,
            input_r: in_r,
        };

        for sender in &self.task_senders {
            if let Err(_) = sender.send(task.clone()) {
                self.drop_count.fetch_add(1, Ordering::Relaxed);
            }
        }

        self.carry_read_ptr = (self.carry_read_ptr + AUDIO_UNIT_SIZE) & mask;
        self.shared_read_ptr
            .store(self.carry_read_ptr, Ordering::Relaxed);
    }

    pub fn get_drop_count(&self) -> usize {
        self.drop_count.load(Ordering::Relaxed)
    }

    pub fn clone_drop_count(&self) -> Arc<AtomicUsize> {
        Arc::clone(&self.drop_count)
    }

    pub fn clone_catch_up_count(&self) -> Arc<AtomicUsize> {
        Arc::clone(&self.catch_up_count)
    }

    pub fn get_catch_up_count(&self) -> usize {
        self.catch_up_count.load(Ordering::Relaxed)
    }
}
