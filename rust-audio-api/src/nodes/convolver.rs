use crate::types::{AUDIO_UNIT_SIZE, AudioUnit};
use dasp::ring_buffer;
use realfft::ComplexToReal;
use realfft::RealFftPlanner;
use realfft::RealToComplex;
use rustfft::num_complex::Complex;
use std::sync::Arc;

type FixedBuf<T> = ring_buffer::Fixed<Vec<T>>;

/// Static configuration for ConvolverNode performance tuning.
pub struct ConvolverConfig {
    /// true = process both L/R channels independently (2x work),
    /// false = mono mode, process L only and copy to R.
    pub stereo: bool,
    /// Multiplier for block_size growth between stages (default 2).
    /// Higher values produce fewer stages for long IRs.
    pub growth_exponent: usize,
    /// Multiplier for the initial stage block size: b = AUDIO_UNIT_SIZE * base_block_multiplier.
    /// Larger values reduce the number of early stages and use direct time-domain
    /// convolution for the first segment, avoiding FFT overhead at startup.
    /// Default is 1 (b = AUDIO_UNIT_SIZE).
    pub base_block_multiplier: usize,
}

impl Default for ConvolverConfig {
    fn default() -> Self {
        Self {
            stereo: true,
            growth_exponent: 2,
            base_block_multiplier: 1,
        }
    }
}

/// Zero-cost skip stage for the initial IR segment.
/// Consumes input and outputs silence — skips convolution entirely.
/// Sacrifices the first 2*block_size samples of the IR for zero CPU cost at startup.
struct SkipStage {
    block_size: usize,
    ir_offset: usize,
    acc_pos: usize,
    output_buf_l: FixedBuf<f32>,
    output_buf_r: Option<FixedBuf<f32>>,
}

impl SkipStage {
    fn new(block_size: usize, ir_offset: usize, stereo: bool) -> Self {
        let mk_fbuf = |n: usize| ring_buffer::Fixed::from(vec![0.0f32; n]);
        Self {
            block_size,
            ir_offset,
            acc_pos: 0,
            output_buf_l: mk_fbuf(block_size),
            output_buf_r: if stereo {
                Some(mk_fbuf(block_size))
            } else {
                None
            },
        }
    }

    #[inline(always)]
    fn feed(&mut self, input_l: &[f32], _input_r: &[f32]) -> bool {
        self.acc_pos += input_l.len();
        if self.acc_pos >= self.block_size {
            self.acc_pos = 0;
            // output_buf stays zeroed — no convolution
            true
        } else {
            false
        }
    }
}

/// A single stage in the NUPC pipeline.
/// Uses overlap-save convolution with realfft for real→complex half-spectrum FFTs.
struct NupcStage {
    r2c: Arc<dyn RealToComplex<f32>>,
    c2r: Arc<dyn ComplexToReal<f32>>,
    /// Flattened IR partitions in frequency domain: num_partitions * freq_size
    ir_freq_l: FixedBuf<Complex<f32>>,
    ir_freq_r: Option<FixedBuf<Complex<f32>>>,
    /// Circular buffer of past FFT'd input blocks: num_partitions * freq_size
    x_fft_ring_l: FixedBuf<Complex<f32>>,
    x_fft_ring_r: Option<FixedBuf<Complex<f32>>>,
    ring_idx: usize,
    num_partitions: usize,
    block_size: usize,
    freq_size: usize, // fft_size / 2 + 1
    /// Scratch buffers (must remain Vec for realfft API compatibility)
    scratch_time_l: Vec<f32>,
    scratch_time_r: Option<Vec<f32>>,
    scratch_freq_l: Vec<Complex<f32>>,
    scratch_freq_r: Option<Vec<Complex<f32>>>,
    /// Accumulator scratch for frequency domain multiply-accumulate
    acc_freq_l: Vec<Complex<f32>>,
    acc_freq_r: Option<Vec<Complex<f32>>>,
    /// The sample offset in the IR that this stage begins at
    ir_offset: usize,
    /// Previous block_size samples carried over for overlap-save
    prev_input_l: FixedBuf<f32>,
    prev_input_r: Option<FixedBuf<f32>>,
    /// Accumulator for collecting AUDIO_UNIT_SIZE chunks into block_size
    input_acc_l: FixedBuf<f32>,
    input_acc_r: Option<FixedBuf<f32>>,
    acc_pos: usize,
    /// Output buffer: block_size samples pre-computed
    output_buf_l: FixedBuf<f32>,
    output_buf_r: Option<FixedBuf<f32>>,
    /// Whether this stage processes stereo
    stereo: bool,
}

impl NupcStage {
    fn new(
        planner: &mut RealFftPlanner<f32>,
        ir_slice: &[[f32; 2]],
        block_size: usize,
        ir_offset: usize,
        stereo: bool,
    ) -> Self {
        let fft_size = 2 * block_size;
        let freq_size = fft_size / 2 + 1;
        let r2c = planner.plan_fft_forward(fft_size);
        let c2r = planner.plan_fft_inverse(fft_size);

        let num_partitions = ((ir_slice.len() + block_size - 1) / block_size).max(1);

        let zero = Complex { re: 0.0, im: 0.0 };
        let mut ir_freq_l = vec![zero; num_partitions * freq_size];
        let mut ir_freq_r = if stereo {
            Some(vec![zero; num_partitions * freq_size])
        } else {
            None
        };

        let norm = fft_size as f32;

        for p in 0..num_partitions {
            let start = p * block_size;
            let end = (start + block_size).min(ir_slice.len());

            let mut time_buf = vec![0.0f32; fft_size];
            let mut freq_buf = vec![zero; freq_size];

            // Left channel
            for i in start..end {
                time_buf[i - start] = ir_slice[i][0] / norm;
            }
            r2c.process(&mut time_buf, &mut freq_buf).unwrap();
            let offset = p * freq_size;
            ir_freq_l[offset..offset + freq_size].copy_from_slice(&freq_buf);

            // Right channel (only if stereo)
            if let Some(ref mut ir_r) = ir_freq_r {
                time_buf.fill(0.0);
                for i in start..end {
                    time_buf[i - start] = ir_slice[i][1] / norm;
                }
                r2c.process(&mut time_buf, &mut freq_buf).unwrap();
                ir_r[offset..offset + freq_size].copy_from_slice(&freq_buf);
            }
        }

        let mk_fbuf = |n: usize| ring_buffer::Fixed::from(vec![0.0f32; n]);
        let mk_cbuf = |n: usize| ring_buffer::Fixed::from(vec![zero; n]);

        Self {
            r2c,
            c2r,
            ir_freq_l: ring_buffer::Fixed::from(ir_freq_l),
            ir_freq_r: ir_freq_r.map(ring_buffer::Fixed::from),
            x_fft_ring_l: mk_cbuf(num_partitions * freq_size),
            x_fft_ring_r: if stereo {
                Some(mk_cbuf(num_partitions * freq_size))
            } else {
                None
            },
            ring_idx: 0,
            num_partitions,
            block_size,
            freq_size,
            scratch_time_l: vec![0.0; fft_size],
            scratch_time_r: if stereo {
                Some(vec![0.0; fft_size])
            } else {
                None
            },
            scratch_freq_l: vec![zero; freq_size],
            scratch_freq_r: if stereo {
                Some(vec![zero; freq_size])
            } else {
                None
            },
            acc_freq_l: vec![zero; freq_size],
            acc_freq_r: if stereo {
                Some(vec![zero; freq_size])
            } else {
                None
            },
            ir_offset,
            prev_input_l: mk_fbuf(block_size),
            prev_input_r: if stereo {
                Some(mk_fbuf(block_size))
            } else {
                None
            },
            input_acc_l: mk_fbuf(block_size),
            input_acc_r: if stereo {
                Some(mk_fbuf(block_size))
            } else {
                None
            },
            acc_pos: 0,
            output_buf_l: mk_fbuf(block_size),
            output_buf_r: if stereo {
                Some(mk_fbuf(block_size))
            } else {
                None
            },
            stereo,
        }
    }

    /// Feed AUDIO_UNIT_SIZE samples. Returns true when a new output block is ready.
    #[inline(always)]
    fn feed(&mut self, input_l: &[f32], input_r: &[f32]) -> bool {
        let n = input_l.len();
        for i in 0..n {
            *self.input_acc_l.get_mut(self.acc_pos + i) = input_l[i];
            if let Some(ref mut acc_r) = self.input_acc_r {
                *acc_r.get_mut(self.acc_pos + i) = input_r[i];
            }
        }
        self.acc_pos += n;

        if self.acc_pos >= self.block_size {
            self.acc_pos = 0;
            self.run_convolution();
            true
        } else {
            false
        }
    }

    /// Process a single channel through the overlap-save pipeline.
    #[inline(always)]
    fn process_channel(
        r2c: &dyn RealToComplex<f32>,
        c2r: &dyn ComplexToReal<f32>,
        scratch_time: &mut [f32],
        scratch_freq: &mut [Complex<f32>],
        acc_freq: &mut [Complex<f32>],
        prev_input: &mut FixedBuf<f32>,
        input_acc: &FixedBuf<f32>,
        x_fft_ring: &mut FixedBuf<Complex<f32>>,
        ir_freq: &FixedBuf<Complex<f32>>,
        output_buf: &mut FixedBuf<f32>,
        bs: usize,
        fs: usize,
        ring_idx: usize,
        num_partitions: usize,
    ) {
        // Build [prev_input | current_input]
        for i in 0..bs {
            scratch_time[i] = *prev_input.get(i);
            scratch_time[bs + i] = *input_acc.get(i);
        }

        // Forward FFT
        r2c.process(scratch_time, scratch_freq).unwrap();

        // Save current input as prev
        for i in 0..bs {
            *prev_input.get_mut(i) = *input_acc.get(i);
        }

        // Store FFT result in ring buffer
        let offset = ring_idx * fs;
        for i in 0..fs {
            *x_fft_ring.get_mut(offset + i) = scratch_freq[i];
        }

        // First partition: direct assignment
        {
            let x_off = ring_idx * fs;
            for i in 0..fs {
                acc_freq[i] = *x_fft_ring.get(x_off + i) * *ir_freq.get(i);
            }
        }

        // Remaining partitions: accumulate
        for p in 1..num_partitions {
            let x_idx = (ring_idx + num_partitions - p) % num_partitions;
            let x_off = x_idx * fs;
            let h_off = p * fs;
            for i in 0..fs {
                acc_freq[i] = acc_freq[i] + *x_fft_ring.get(x_off + i) * *ir_freq.get(h_off + i);
            }
        }

        // Inverse FFT
        c2r.process(acc_freq, scratch_time).unwrap();

        // Extract valid region
        for i in 0..bs {
            *output_buf.get_mut(i) = scratch_time[bs + i];
        }
    }

    /// Run the overlap-save convolution for one block using realfft.
    fn run_convolution(&mut self) {
        let bs = self.block_size;
        let fs = self.freq_size;
        let ring_idx = self.ring_idx;
        let np = self.num_partitions;

        // Left channel (always processed)
        Self::process_channel(
            self.r2c.as_ref(),
            self.c2r.as_ref(),
            &mut self.scratch_time_l,
            &mut self.scratch_freq_l,
            &mut self.acc_freq_l,
            &mut self.prev_input_l,
            &self.input_acc_l,
            &mut self.x_fft_ring_l,
            &self.ir_freq_l,
            &mut self.output_buf_l,
            bs,
            fs,
            ring_idx,
            np,
        );

        // Right channel (only if stereo)
        if self.stereo {
            Self::process_channel(
                self.r2c.as_ref(),
                self.c2r.as_ref(),
                self.scratch_time_r.as_mut().unwrap(),
                self.scratch_freq_r.as_mut().unwrap(),
                self.acc_freq_r.as_mut().unwrap(),
                self.prev_input_r.as_mut().unwrap(),
                self.input_acc_r.as_ref().unwrap(),
                self.x_fft_ring_r.as_mut().unwrap(),
                self.ir_freq_r.as_ref().unwrap(),
                self.output_buf_r.as_mut().unwrap(),
                bs,
                fs,
                ring_idx,
                np,
            );
        }

        self.ring_idx = (self.ring_idx + 1) % self.num_partitions;
    }
}

/// Stage type: either skip (no-op) or FFT-based NUPC.
enum StageKind {
    Skip(SkipStage),
    Fft(NupcStage),
}

impl StageKind {
    fn block_size(&self) -> usize {
        match self {
            StageKind::Skip(s) => s.block_size,
            StageKind::Fft(s) => s.block_size,
        }
    }

    fn ir_offset(&self) -> usize {
        match self {
            StageKind::Skip(s) => s.ir_offset,
            StageKind::Fft(s) => s.ir_offset,
        }
    }

    fn feed(&mut self, input_l: &[f32], input_r: &[f32]) -> bool {
        match self {
            StageKind::Skip(s) => s.feed(input_l, input_r),
            StageKind::Fft(s) => s.feed(input_l, input_r),
        }
    }

    fn output_buf_l(&self) -> &FixedBuf<f32> {
        match self {
            StageKind::Skip(s) => &s.output_buf_l,
            StageKind::Fft(s) => &s.output_buf_l,
        }
    }

    fn output_buf_r(&self) -> Option<&FixedBuf<f32>> {
        match self {
            StageKind::Skip(s) => s.output_buf_r.as_ref(),
            StageKind::Fft(s) => s.output_buf_r.as_ref(),
        }
    }
}

pub struct ConvolverNode {
    stages: Vec<StageKind>,
    /// Output accumulation ring buffer using dasp Fixed ring buffer
    output_ring_l: FixedBuf<f32>,
    output_ring_r: FixedBuf<f32>,
    /// Global sample counter (in units of AUDIO_UNIT_SIZE blocks)
    block_counter: usize,
    /// Whether processing in stereo mode
    stereo: bool,
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
            println!(
                "IR 採樣率 ({}) 與目標採樣率 ({}) 不同，進行重採樣...",
                spec.sample_rate, target_sample_rate
            );
            Self::resample_ir(&ir, spec.sample_rate, target_sample_rate)
        } else {
            println!(
                "IR 採樣率 ({}) 與目標採樣率 ({}) 相同，直接使用...",
                spec.sample_rate, target_sample_rate
            );
            ir
        };

        if let Some(max) = max_len {
            if ir.len() > max {
                println!("IR 過長 ({} samples)，自動截斷為 {} samples", ir.len(), max);
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
        let mut planner = RealFftPlanner::new();
        let mut stages: Vec<StageKind> = Vec::new();
        let stereo = config.stereo;
        let growth = config.growth_exponent.max(2);
        let base_mult = config.base_block_multiplier.max(1);

        if ir.is_empty() {
            stages.push(StageKind::Skip(SkipStage::new(AUDIO_UNIT_SIZE, 0, stereo)));
            return Self {
                stages,
                output_ring_l: ring_buffer::Fixed::from(vec![0.0f32; 65536]),
                output_ring_r: ring_buffer::Fixed::from(vec![0.0f32; 65536]),
                block_counter: 0,
                stereo,
            };
        }

        // Gardner-style NUPC partition layout with direct convolution for stage 0.
        let b = AUDIO_UNIT_SIZE * base_mult;
        let max_block_size = AUDIO_UNIT_SIZE * 128;
        let ir_len = ir.len();
        let mut offset = 0;
        let mut block_size = b;
        let mut stage_idx = 0;

        while offset < ir_len {
            let stage_ir_len = if stage_idx == 0 {
                2 * block_size
            } else if block_size < max_block_size {
                block_size
            } else {
                ir_len - offset
            };

            let end = (offset + stage_ir_len).min(ir_len);
            let stage_ir = &ir[offset..end];

            if stage_idx == 0 && base_mult > 1 {
                // Skip stage: zero CPU cost, sacrifices IR[0..2*b] early reflections
                stages.push(StageKind::Skip(SkipStage::new(block_size, offset, stereo)));
            } else {
                stages.push(StageKind::Fft(NupcStage::new(
                    &mut planner,
                    stage_ir,
                    block_size,
                    offset,
                    stereo,
                )));
            }

            offset = end;
            if stage_idx == 0 {
                block_size = growth * b;
            } else if block_size < max_block_size {
                block_size = (block_size * growth).min(max_block_size);
            }
            stage_idx += 1;
        }

        Self {
            stages,
            output_ring_l: ring_buffer::Fixed::from(vec![0.0f32; 65536]),
            output_ring_r: ring_buffer::Fixed::from(vec![0.0f32; 65536]),
            block_counter: 0,
            stereo,
        }
    }

    #[inline(always)]
    pub fn process(&mut self, input: Option<&AudioUnit>, output: &mut AudioUnit) {
        let empty_input = crate::types::empty_audio_unit();
        let input_ref = input.unwrap_or(&empty_input);

        // Deinterleave input
        let mut in_l = [0.0f32; AUDIO_UNIT_SIZE];
        let mut in_r = [0.0f32; AUDIO_UNIT_SIZE];
        for i in 0..AUDIO_UNIT_SIZE {
            in_l[i] = input_ref[i][0];
            in_r[i] = input_ref[i][1];
        }

        let write_base = self.block_counter * AUDIO_UNIT_SIZE;
        let ring_len = self.output_ring_l.len();

        // Feed input to all stages and collect their output
        for stage in self.stages.iter_mut() {
            let ready = stage.feed(&in_l, &in_r);
            if ready {
                let stage_output_start =
                    write_base + AUDIO_UNIT_SIZE - stage.block_size() + stage.ir_offset();

                let bs = stage.block_size();
                let out_l = stage.output_buf_l();
                let out_r = stage.output_buf_r();

                for i in 0..bs {
                    let idx = (stage_output_start + i) % ring_len;
                    *self.output_ring_l.get_mut(idx) += *out_l.get(i);
                    if let Some(ref buf_r) = out_r {
                        *self.output_ring_r.get_mut(idx) += *buf_r.get(i);
                    }
                }
            }
        }

        // Read output from ring buffer and clear
        for i in 0..AUDIO_UNIT_SIZE {
            let idx = (write_base + i) % ring_len;
            output[i][0] = *self.output_ring_l.get(idx);
            output[i][1] = if self.stereo {
                *self.output_ring_r.get(idx)
            } else {
                // Mono mode: copy L to R
                output[i][0]
            };
            *self.output_ring_l.get_mut(idx) = 0.0;
            *self.output_ring_r.get_mut(idx) = 0.0;
        }

        self.block_counter += 1;
    }
}
