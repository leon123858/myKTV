use crate::types::{AUDIO_UNIT_SIZE, AudioUnit};
use crossbeam_queue::ArrayQueue;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};

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
    fft_data_l: Vec<rustfft::num_complex::Complex<f32>>,
    fft_data_r: Vec<rustfft::num_complex::Complex<f32>>,
}

struct TaskMsg {
    unit_index: u64,
    carry_read_ptr: usize,
    input_l: [f32; AUDIO_UNIT_SIZE],
    input_r: [f32; AUDIO_UNIT_SIZE],
}

pub struct ConvolverNode {
    stereo: bool,
    block_0_l: [f32; AUDIO_UNIT_SIZE],
    block_0_r: [f32; AUDIO_UNIT_SIZE],
    task_queue: Arc<ArrayQueue<TaskMsg>>,
    carry_buffer_l: Arc<Vec<AtomicF32>>,
    carry_buffer_r: Arc<Vec<AtomicF32>>,
    carry_mask: usize,
    carry_read_ptr: usize,
    unit_counter: Arc<AtomicU64>,
    drop_count: Arc<AtomicUsize>,
    is_alive: Arc<AtomicBool>,
}

impl Drop for ConvolverNode {
    fn drop(&mut self) {
        self.is_alive.store(false, Ordering::Relaxed);
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

        let task_queue = Arc::new(ArrayQueue::<TaskMsg>::new(2048));
        let drop_count = Arc::new(AtomicUsize::new(0));
        let unit_counter = Arc::new(AtomicU64::new(0));
        let is_alive = Arc::new(AtomicBool::new(true));

        let mut b0_l = [0.0f32; AUDIO_UNIT_SIZE];
        let mut b0_r = [0.0f32; AUDIO_UNIT_SIZE];
        if !blocks.is_empty() {
            b0_l.copy_from_slice(&blocks[0].time_domain_l[..AUDIO_UNIT_SIZE]);
            b0_r.copy_from_slice(&blocks[0].time_domain_r[..AUDIO_UNIT_SIZE]);
        }

        let worker_task_queue = Arc::clone(&task_queue);
        let worker_carry_l = Arc::clone(&carry_buffer_l);
        let worker_carry_r = Arc::clone(&carry_buffer_r);
        let worker_drop_count = Arc::clone(&drop_count);
        let worker_unit_counter = Arc::clone(&unit_counter);
        let worker_is_alive = Arc::clone(&is_alive);

        let hist_cap = max_block_size.max(AUDIO_UNIT_SIZE);
        let worker_stereo = stereo;

        std::thread::spawn(move || {
            let mut head = 0;
            let mut history_l = vec![0.0f32; hist_cap];
            let mut history_r = vec![0.0f32; hist_cap];

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

            let max_fft_len = max_block_size * 2;
            let mut padded_l = vec![0.0f32; max_fft_len];
            let mut padded_r = vec![0.0f32; max_fft_len];
            let max_out_len = max_block_size + 1;
            let mut out_l = vec![rustfft::num_complex::Complex::new(0.0, 0.0); max_out_len];
            let mut out_r = vec![rustfft::num_complex::Complex::new(0.0, 0.0); max_out_len];
            let mut result_l = vec![0.0f32; max_fft_len];
            let mut result_r = vec![0.0f32; max_fft_len];

            while worker_is_alive.load(Ordering::Relaxed) {
                if let Some(task) = worker_task_queue.pop() {
                    let latest_unit = worker_unit_counter.load(Ordering::Acquire);
                    let age = latest_unit.saturating_sub(task.unit_index);
                    let mut dropped = false;

                    if age > 4 {
                        dropped = true;
                        worker_drop_count.fetch_add(1, Ordering::Relaxed);
                    }

                    for i in 0..AUDIO_UNIT_SIZE {
                        history_l[(head + i) % hist_cap] = task.input_l[i];
                        if worker_stereo {
                            history_r[(head + i) % hist_cap] = task.input_r[i];
                        }
                    }
                    head = (head + AUDIO_UNIT_SIZE) % hist_cap;

                    if dropped {
                        continue;
                    }

                    let mut blocks_iter = blocks.iter().skip(1).peekable();
                    while let Some(block) = blocks_iter.next() {
                        let units_needed = block.size / AUDIO_UNIT_SIZE;
                        if (task.unit_index + 1) % units_needed as u64 == 0 {
                            let s = block.size;
                            let len2 = s * 2;
                            let out_len = s + 1;
                            let (fft, ifft) = fft_plans.get(&len2).unwrap();

                            let start_idx = (head + hist_cap - s) % hist_cap;

                            let pad_l = &mut padded_l[..len2];
                            pad_l.fill(0.0);
                            for i in 0..s {
                                pad_l[i] = history_l[(start_idx + i) % hist_cap];
                            }

                            let pad_r = &mut padded_r[..len2];
                            if worker_stereo {
                                pad_r.fill(0.0);
                                for i in 0..s {
                                    pad_r[i] = history_r[(start_idx + i) % hist_cap];
                                }
                            }

                            let out_l_slice = &mut out_l[..out_len];
                            fft.process(pad_l, out_l_slice).unwrap();

                            let out_r_slice = &mut out_r[..out_len];
                            if worker_stereo {
                                fft.process(pad_r, out_r_slice).unwrap();
                            }

                            for i in 0..out_len {
                                out_l_slice[i] = out_l_slice[i] * block.fft_data_l[i];
                                if worker_stereo {
                                    out_r_slice[i] = out_r_slice[i] * block.fft_data_r[i];
                                }
                            }

                            let res_l = &mut result_l[..len2];
                            ifft.process(out_l_slice, res_l).unwrap();
                            let scale = 1.0 / (len2 as f32);
                            for x in res_l.iter_mut() {
                                *x *= scale;
                            }

                            let res_r = &mut result_r[..len2];
                            if worker_stereo {
                                ifft.process(out_r_slice, res_r).unwrap();
                                for x in res_r.iter_mut() {
                                    *x *= scale;
                                }
                            }

                            let cap = capacity;
                            let base_ptr = (task.carry_read_ptr + cap
                                - ((units_needed - 1) * AUDIO_UNIT_SIZE) % cap)
                                % cap;
                            let out_base = (base_ptr + block.offset) % cap;

                            for i in 0..(len2 - 1) {
                                let idx = (out_base + i) & carry_mask;
                                worker_carry_l[idx].fetch_add(res_l[i], Ordering::Relaxed);
                                if worker_stereo {
                                    worker_carry_r[idx].fetch_add(res_r[i], Ordering::Relaxed);
                                }
                            }
                        }
                    }
                } else {
                    std::thread::yield_now();
                    std::thread::sleep(std::time::Duration::from_micros(20));
                }
            }
        });

        Self {
            stereo,
            block_0_l: b0_l,
            block_0_r: b0_r,
            task_queue,
            carry_buffer_l,
            carry_buffer_r,
            carry_mask,
            carry_read_ptr: 0,
            unit_counter,
            drop_count,
            is_alive,
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
            fft_data_l: vec![],
            fft_data_r: vec![],
        });
        offset += b0_len;

        let mut current_size = AUDIO_UNIT_SIZE;
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
                fft_data_l: out_l,
                fft_data_r: out_r,
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

        let unit_idx = self.unit_counter.fetch_add(1, Ordering::SeqCst);
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
            unit_index: unit_idx,
            carry_read_ptr: self.carry_read_ptr,
            input_l: in_l,
            input_r: in_r,
        };

        if let Err(_) = self.task_queue.push(task) {
            self.drop_count.fetch_add(1, Ordering::Relaxed);
        }

        self.carry_read_ptr = (self.carry_read_ptr + AUDIO_UNIT_SIZE) & mask;
    }

    pub fn get_drop_count(&self) -> usize {
        self.drop_count.load(Ordering::Relaxed)
    }

    pub fn clone_drop_count(&self) -> Arc<AtomicUsize> {
        Arc::clone(&self.drop_count)
    }
}
