use crate::types::{AudioUnit, AUDIO_UNIT_SIZE};
use rustfft::{Fft, FftPlanner, num_complex::Complex};
use std::sync::Arc;

pub struct ConvolverNode {
    fft: Arc<dyn Fft<f32>>,
    ifft: Arc<dyn Fft<f32>>,
    ir_freq_l: Vec<Complex<f32>>,
    ir_freq_r: Vec<Complex<f32>>,
    overlap_l: Vec<f32>,
    overlap_r: Vec<f32>,
    fft_size: usize,
    scratch_l: Vec<Complex<f32>>,
    scratch_r: Vec<Complex<f32>>,
    fft_scratch: Vec<Complex<f32>>,
}

impl ConvolverNode {
    pub fn from_file(path: &str, max_len: Option<usize>) -> anyhow::Result<Self> {
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
            let max_val = if spec.bits_per_sample == 16 {
                32768.0
            } else {
                8388608.0
            }; // Assume 16 or 24 bit
            let mut iter = reader.samples::<i32>();
            while let Some(Ok(l)) = iter.next() {
                let r = if spec.channels == 2 {
                    iter.next().unwrap().unwrap_or(l)
                } else {
                    l
                };
                ir.push([l as f32 / max_val, r as f32 / max_val]);
            }
        }

        if let Some(max) = max_len {
            if ir.len() > max {
                println!("IR 過長 ({} samples)，自動截斷為 {} samples", ir.len(), max);
                ir.truncate(max);
            }
        }

        Ok(Self::new(&ir))
    }

    pub fn new(ir: &[[f32; 2]]) -> Self {
        let ir_len = ir.len();
        // Convolution of lengths N and M requires at least N + M - 1 for linear convolution
        let target_len = AUDIO_UNIT_SIZE + ir_len - 1;
        let fft_size = target_len.next_power_of_two();

        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(fft_size);
        let ifft = planner.plan_fft_inverse(fft_size);

        let mut ir_freq_l = vec![Complex { re: 0.0, im: 0.0 }; fft_size];
        let mut ir_freq_r = vec![Complex { re: 0.0, im: 0.0 }; fft_size];

        for i in 0..ir_len {
            ir_freq_l[i].re = ir[i][0];
            ir_freq_r[i].re = ir[i][1];
        }

        let mut fft_scratch = vec![Complex { re: 0.0, im: 0.0 }; fft.get_inplace_scratch_len()];
        fft.process_with_scratch(&mut ir_freq_l, &mut fft_scratch);
        fft.process_with_scratch(&mut ir_freq_r, &mut fft_scratch);

        let overlap_size = fft_size.saturating_sub(AUDIO_UNIT_SIZE);

        Self {
            fft,
            ifft,
            ir_freq_l,
            ir_freq_r,
            overlap_l: vec![0.0; overlap_size],
            overlap_r: vec![0.0; overlap_size],
            fft_size,
            scratch_l: vec![Complex { re: 0.0, im: 0.0 }; fft_size],
            scratch_r: vec![Complex { re: 0.0, im: 0.0 }; fft_size],
            fft_scratch,
        }
    }

    #[inline(always)]
    pub fn process(&mut self, input: Option<&AudioUnit>, output: &mut AudioUnit) {
        let empty_input = crate::types::empty_audio_unit();
        let input_ref = input.unwrap_or(&empty_input);

        // 1. Copy input to scratch and pad with 0
        for i in 0..AUDIO_UNIT_SIZE {
            self.scratch_l[i] = Complex { re: input_ref[i][0], im: 0.0 };
            self.scratch_r[i] = Complex { re: input_ref[i][1], im: 0.0 };
        }
        for i in AUDIO_UNIT_SIZE..self.fft_size {
            self.scratch_l[i] = Complex { re: 0.0, im: 0.0 };
            self.scratch_r[i] = Complex { re: 0.0, im: 0.0 };
        }

        // 2. Forward FFT
        self.fft.process_with_scratch(&mut self.scratch_l, &mut self.fft_scratch);
        self.fft.process_with_scratch(&mut self.scratch_r, &mut self.fft_scratch);

        // 3. Multiply by IR freq
        for i in 0..self.fft_size {
            self.scratch_l[i] = self.scratch_l[i] * self.ir_freq_l[i];
            self.scratch_r[i] = self.scratch_r[i] * self.ir_freq_r[i];
        }

        // 4. Inverse FFT
        self.ifft.process_with_scratch(&mut self.scratch_l, &mut self.fft_scratch);
        self.ifft.process_with_scratch(&mut self.scratch_r, &mut self.fft_scratch);

        // 5. Normalize, Add overlap, and Output
        let norm = self.fft_size as f32;
        let overlap_size = self.fft_size - AUDIO_UNIT_SIZE;

        for i in 0..self.fft_size {
            self.scratch_l[i].re /= norm;
            self.scratch_r[i].re /= norm;

            if i < overlap_size {
                self.scratch_l[i].re += self.overlap_l[i];
                self.scratch_r[i].re += self.overlap_r[i];
            }
        }

        for i in 0..AUDIO_UNIT_SIZE {
            output[i][0] = self.scratch_l[i].re;
            output[i][1] = self.scratch_r[i].re;
        }

        for i in 0..overlap_size {
            self.overlap_l[i] = self.scratch_l[i + AUDIO_UNIT_SIZE].re;
            self.overlap_r[i] = self.scratch_r[i + AUDIO_UNIT_SIZE].re;
        }
    }
}
