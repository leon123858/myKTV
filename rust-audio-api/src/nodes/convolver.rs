use crate::types::{AUDIO_UNIT_SIZE, AudioUnit};
use crossbeam_channel::{Sender, bounded};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use thread_priority::*;

/// Configuration for the [`ConvolverNode`].
pub struct ConvolverConfig {
    /// Whether the convolution should be performed in stereo.
    pub stereo: bool,
    /// The exponent for partitioning the Impulse Response (IR).
    /// Controls how the IR is divided into blocks for processing.
    pub growth_exponent: u32,
    /// The size of the first block (Block 0) in samples.
    pub block_0_size: usize,
}

impl Default for ConvolverConfig {
    fn default() -> Self {
        Self {
            stereo: true,
            growth_exponent: 2,
            block_0_size: AUDIO_UNIT_SIZE * 4,
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
    pub fn load(&self, order: Ordering) -> f32 {
        f32::from_bits(self.0.load(order))
    }

    #[inline(always)]
    pub fn store(&self, val: f32, order: Ordering) {
        self.0.store(val.to_bits(), order);
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
    fft_data_l: Arc<[rustfft::num_complex::Complex<f32>]>,
    fft_data_r: Arc<[rustfft::num_complex::Complex<f32>]>,
    fft_plan: Arc<dyn realfft::RealToComplex<f32>>,
    ifft_plan: Arc<dyn realfft::ComplexToReal<f32>>,
}

#[derive(Clone, Copy)]
struct TaskMsg {
    block_index: usize,
    carry_read_ptr: usize,
    history_write_ptr: usize,
}

/// A node that performs real-time convolution against an Impulse Response (IR).
///
/// Convolver is used for effects like reverb, speaker modeling, or virtual acoustics.
/// It uses a partitioned convolution algorithm with background worker threads to
/// achieve low-latency performance even with long IRs.
pub struct ConvolverNode {
    stereo: bool,
    block_0_l: Vec<f32>,
    block_0_r: Vec<f32>,
    b0_out_l: Vec<f32>,
    b0_out_r: Vec<f32>,
    task_tx: Sender<TaskMsg>,

    carry_buffer_l: Arc<Vec<AtomicF32>>,
    carry_buffer_r: Arc<Vec<AtomicF32>>,
    carry_mask: usize,
    carry_read_ptr: usize,

    history_buffer_l: Arc<Vec<AtomicF32>>,
    history_buffer_r: Arc<Vec<AtomicF32>>,
    history_mask: usize,
    history_write_ptr: usize,

    partition_blocks: Arc<[PartitionBlock]>,

    shared_read_ptr: Arc<AtomicUsize>,
    drop_count: Arc<AtomicUsize>,
}

impl ConvolverNode {
    /// Creates a `ConvolverNode` by loading an Impulse Response (IR) from a WAV file.
    ///
    /// # Parameters
    /// - `path`: Path to the WAV file.
    /// - `target_sample_rate`: Target sample rate for processing.
    /// - `max_len`: Optional maximum length (in samples) to truncate the IR.
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

    /// Creates a `ConvolverNode` from a WAV file with custom configuration.
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
            panic!("Unexpected IR file format")
        }

        let mut ir = if spec.sample_rate != target_sample_rate {
            Self::resample_ir(&ir, spec.sample_rate, target_sample_rate)
        } else {
            ir
        };

        if let Some(max) = max_len
            && ir.len() > max {
                ir.truncate(max);

                // Apply fade-out to avoid artifacts from abrupt truncation (fade-out last 100ms)
                let fade_len = (target_sample_rate as f32 * 0.1) as usize;
                let fade_len = fade_len.min(max);
                for i in 0..fade_len {
                    let idx = max - 1 - i;
                    let fade_gain = i as f32 / fade_len as f32;
                    // Use exponential or smooth fade-out; here we use simple linear
                    ir[idx][0] *= fade_gain;
                    ir[idx][1] *= fade_gain;
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
        let (b0_l_vec, b0_r_vec, blocks_info) =
            Self::partition_ir(ir, config.growth_exponent, config.block_0_size);

        let max_block_size = blocks_info
            .last()
            .map(|b| b.size)
            .unwrap_or(AUDIO_UNIT_SIZE);
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

        let mut history_capacity = max_block_size.next_power_of_two();
        if history_capacity < 65536 {
            history_capacity = 65536;
        }
        let history_mask = history_capacity - 1;

        let history_buffer_l = Arc::new(
            (0..history_capacity)
                .map(|_| AtomicF32::new(0.0))
                .collect::<Vec<_>>(),
        );
        let history_buffer_r = Arc::new(
            (0..history_capacity)
                .map(|_| AtomicF32::new(0.0))
                .collect::<Vec<_>>(),
        );

        let drop_count = Arc::new(AtomicUsize::new(0));
        let shared_read_ptr = Arc::new(AtomicUsize::new(0));

        let b0_l = b0_l_vec.clone();
        let b0_r = b0_r_vec.clone();
        let b0_out_len = AUDIO_UNIT_SIZE + config.block_0_size - 1;
        let b0_out_l = vec![0.0f32; b0_out_len];
        let b0_out_r = vec![0.0f32; b0_out_len];

        let max_queue_len = 2048;
        let (task_tx, rx) = bounded::<TaskMsg>(max_queue_len);

        let partition_blocks: Arc<[PartitionBlock]> = blocks_info.into();
        let num_workers = std::thread::available_parallelism()
            .map(|x| x.get())
            .unwrap_or(4);

        for _ in 0..num_workers {
            let rx = rx.clone();
            let worker_carry_l = Arc::clone(&carry_buffer_l);
            let worker_carry_r = Arc::clone(&carry_buffer_r);
            let worker_hist_l = Arc::clone(&history_buffer_l);
            let worker_hist_r = Arc::clone(&history_buffer_r);
            let worker_drop_count = Arc::clone(&drop_count);
            let worker_shared_read_ptr = Arc::clone(&shared_read_ptr);
            let worker_blocks = Arc::clone(&partition_blocks);
            let worker_stereo = stereo;
            let global_hist_cap = history_capacity;
            let global_hist_mask = history_mask;
            let global_carry_mask = carry_mask;

            std::thread::spawn(move || {
                if let Err(e) = set_current_thread_priority(ThreadPriority::Max) {
                    eprintln!(
                        "Warning: Failed to set convolution block thread priority: {:?}",
                        e
                    );
                }

                let max_len2 = max_block_size * 2;
                let max_out_len = max_block_size + 1;

                let mut pad_l = vec![0.0f32; max_len2];
                let mut pad_r = vec![0.0f32; max_len2];
                let mut out_l_slice =
                    vec![rustfft::num_complex::Complex::new(0.0, 0.0); max_out_len];
                let mut out_r_slice =
                    vec![rustfft::num_complex::Complex::new(0.0, 0.0); max_out_len];
                let mut res_l = vec![0.0f32; max_len2];
                let mut res_r = vec![0.0f32; max_len2];

                while let Ok(task) = rx.recv() {
                    let queue_len = rx.len();

                    let max_queue_age = if queue_len > max_queue_len / 2 { 2 } else { 8 };
                    if queue_len > max_queue_age {
                        worker_drop_count.fetch_add(1, Ordering::Relaxed);
                        continue;
                    }

                    let block = &worker_blocks[task.block_index];
                    let s = block.size;
                    let len2 = s * 2;
                    let out_len = s + 1;

                    let start_idx =
                        (task.history_write_ptr + global_hist_cap - s) & global_hist_mask;

                    for i in 0..s {
                        pad_l[i] = worker_hist_l[(start_idx + i) & global_hist_mask]
                            .load(Ordering::Relaxed);
                    }
                    pad_l[s..len2].fill(0.0);

                    if worker_stereo {
                        for i in 0..s {
                            pad_r[i] = worker_hist_r[(start_idx + i) & global_hist_mask]
                                .load(Ordering::Relaxed);
                        }
                        pad_r[s..len2].fill(0.0);
                    }

                    let pad_l_slice = &mut pad_l[..len2];
                    block
                        .fft_plan
                        .process(pad_l_slice, &mut out_l_slice[..out_len])
                        .unwrap();

                    if worker_stereo {
                        let pad_r_slice = &mut pad_r[..len2];
                        block
                            .fft_plan
                            .process(pad_r_slice, &mut out_r_slice[..out_len])
                            .unwrap();
                    }

                    for i in 0..out_len {
                        out_l_slice[i] *= block.fft_data_l[i];
                        if worker_stereo {
                            out_r_slice[i] *= block.fft_data_r[i];
                        }
                    }

                    let res_l_mut = &mut res_l[..len2];
                    block
                        .ifft_plan
                        .process(&mut out_l_slice[..out_len], res_l_mut)
                        .unwrap();
                    let scale = 1.0 / (len2 as f32);
                    for x in res_l_mut.iter_mut() {
                        *x *= scale;
                    }

                    if worker_stereo {
                        let res_r_mut = &mut res_r[..len2];
                        block
                            .ifft_plan
                            .process(&mut out_r_slice[..out_len], res_r_mut)
                            .unwrap();
                        for x in res_r_mut.iter_mut() {
                            *x *= scale;
                        }
                    }

                    let current_ptr = worker_shared_read_ptr.load(Ordering::Relaxed);
                    let task_ptr = task.carry_read_ptr;
                    let capacity = global_carry_mask + 1;

                    let current_real = if current_ptr < task_ptr {
                        current_ptr + capacity
                    } else {
                        current_ptr
                    };

                    let out_base_real =
                        (task_ptr + AUDIO_UNIT_SIZE + block.offset).saturating_sub(s);
                    let safe_current_real = current_real + AUDIO_UNIT_SIZE;

                    let skip = safe_current_real.saturating_sub(out_base_real);

                    const FADE_LEN: usize = AUDIO_UNIT_SIZE / 4;

                    for i in skip..(len2 - 1) {
                        let mut sample_l = res_l[i];
                        let mut sample_r = res_r[i];

                        // fade in
                        let current_offset = i - skip;
                        if current_offset < FADE_LEN {
                            let gain = current_offset as f32 / FADE_LEN as f32;
                            sample_l *= gain;
                            if worker_stereo {
                                sample_r *= gain;
                            }
                        }

                        let idx = (out_base_real + i) & global_carry_mask;
                        worker_carry_l[idx].fetch_add(sample_l, Ordering::Relaxed);
                        if worker_stereo {
                            worker_carry_r[idx].fetch_add(sample_r, Ordering::Relaxed);
                        }
                    }
                }
            });
        }

        Self {
            stereo,
            block_0_l: b0_l,
            block_0_r: b0_r,
            b0_out_l,
            b0_out_r,
            task_tx,
            carry_buffer_l,
            carry_buffer_r,
            carry_mask,
            carry_read_ptr: 0,

            history_buffer_l,
            history_buffer_r,
            history_mask,
            history_write_ptr: 0,

            partition_blocks,

            shared_read_ptr,
            drop_count,
        }
    }

    fn partition_ir(
        ir: &[[f32; 2]],
        growth_exponent: u32,
        b0_len: usize,
    ) -> (Vec<f32>, Vec<f32>, Vec<PartitionBlock>) {
        let mut blocks = Vec::new();
        let mut offset = 0;
        let growth_factor = growth_exponent.max(1) as usize;

        let b0_l = Self::take_slice_padded(ir, offset, b0_len, 0);
        let b0_r = Self::take_slice_padded(ir, offset, b0_len, 1);
        offset += b0_len;

        let mut current_size = AUDIO_UNIT_SIZE * (growth_exponent as usize);
        let mut planner = realfft::RealFftPlanner::<f32>::new();

        while offset < ir.len() {
            let len = current_size;
            let len2 = len * 2;
            let l_slice = Self::take_slice_padded(ir, offset, len, 0);
            let r_slice = Self::take_slice_padded(ir, offset, len, 1);

            let fft_fwd = planner.plan_fft_forward(len2);
            let fft_inv = planner.plan_fft_inverse(len2);

            let mut padded_l = vec![0.0; len2];
            padded_l[..len].copy_from_slice(&l_slice);
            let mut out_l = fft_fwd.make_output_vec();
            fft_fwd.process(&mut padded_l, &mut out_l).unwrap();

            let mut padded_r = vec![0.0; len2];
            padded_r[..len].copy_from_slice(&r_slice);
            let mut out_r = fft_fwd.make_output_vec();
            fft_fwd.process(&mut padded_r, &mut out_r).unwrap();

            blocks.push(PartitionBlock {
                size: len,
                offset,
                fft_data_l: out_l.into(),
                fft_data_r: out_r.into(),
                fft_plan: fft_fwd,
                ifft_plan: fft_inv,
            });

            offset += len;

            // offset represents current input audio length
            // current_size * growth_factor + AUDIO_UNIT_SIZE represents the threshold for growth
            // When this condition is met, the new size can be calculated without waiting,
            // preventing the block from becoming too large and starving the main thread.
            if offset >= current_size * growth_factor + AUDIO_UNIT_SIZE {
                current_size *= growth_factor;
            }
        }
        (b0_l, b0_r, blocks)
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

        for i in 0..AUDIO_UNIT_SIZE {
            let idx = (self.history_write_ptr + i) & self.history_mask;
            self.history_buffer_l[idx].store(in_l[i], Ordering::Relaxed);
            if self.stereo {
                self.history_buffer_r[idx].store(in_r[i], Ordering::Relaxed);
            }
        }
        self.history_write_ptr = (self.history_write_ptr + AUDIO_UNIT_SIZE) & self.history_mask;

        let mask = self.carry_mask;

        let b0_len = self.block_0_l.len();
        let b0_out_len = AUDIO_UNIT_SIZE + b0_len - 1;
        self.b0_out_l[..b0_out_len].fill(0.0);
        if self.stereo {
            self.b0_out_r[..b0_out_len].fill(0.0);
        }

        for i in 0..AUDIO_UNIT_SIZE {
            let il = in_l[i];
            let ir = in_r[i];
            let out_l_slice = &mut self.b0_out_l[i..i + b0_len];

            for (out_l, &b0l) in out_l_slice.iter_mut().zip(self.block_0_l.iter()) {
                *out_l += il * b0l;
            }
            if self.stereo {
                let out_r_slice = &mut self.b0_out_r[i..i + b0_len];
                for (out_r, &b0r) in out_r_slice.iter_mut().zip(self.block_0_r.iter()) {
                    *out_r += ir * b0r;
                }
            }
        }

        for i in 0..b0_out_len {
            let idx = (self.carry_read_ptr + i) & mask;
            self.carry_buffer_l[idx].fetch_add(self.b0_out_l[i], Ordering::Relaxed);
            if self.stereo {
                self.carry_buffer_r[idx].fetch_add(self.b0_out_r[i], Ordering::Relaxed);
            }
        }

        for (i, out) in output.iter_mut().enumerate().take(AUDIO_UNIT_SIZE) {
            let idx = (self.carry_read_ptr + i) & mask;
            let out_l = self.carry_buffer_l[idx].swap(0.0, Ordering::Relaxed);
            let out_r = self.carry_buffer_r[idx].swap(0.0, Ordering::Relaxed);

            out[0] = out_l;
            if self.stereo {
                out[1] = out_r;
            } else {
                out[1] = out_l;
            }
        }

        for (idx, block) in self.partition_blocks.iter().enumerate() {
            if (self.carry_read_ptr + AUDIO_UNIT_SIZE).is_multiple_of(block.size) {
                let task = TaskMsg {
                    block_index: idx,
                    carry_read_ptr: self.carry_read_ptr,
                    history_write_ptr: self.history_write_ptr,
                };
                if self.task_tx.try_send(task).is_err() {
                    self.drop_count.fetch_add(1, Ordering::Relaxed);
                }
            }
        }

        self.carry_read_ptr = (self.carry_read_ptr + AUDIO_UNIT_SIZE) & mask;
        self.shared_read_ptr
            .store(self.carry_read_ptr, Ordering::Relaxed);
    }

    pub fn get_drop_count(&self) -> usize {
        self.drop_count.load(Ordering::Relaxed)
    }

    /// Returns a shared reference to the drop count.
    ///
    /// Drop count increases when the convolution worker threads fall behind
    /// the real-time audio thread.
    pub fn clone_drop_count(&self) -> Arc<AtomicUsize> {
        Arc::clone(&self.drop_count)
    }
}
