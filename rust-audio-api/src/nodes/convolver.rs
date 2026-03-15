use crate::types::{AUDIO_UNIT_SIZE, AudioUnit};
use dasp::ring_buffer;

type FixedBuf<T> = ring_buffer::Fixed<Vec<T>>;

/// Static configuration for ConvolverNode performance tuning.
pub struct ConvolverConfig {
    /// true = process both L/R channels independently (2x work),
    /// false = mono mode, process L only and copy to R.
    pub stereo: bool,
    /// Growth exponent for block size (2 = exponential growth)
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

pub struct ConvolverNode {
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
        let stereo = config.stereo;

        if ir.is_empty() {
            return Self {
                output_ring_l: ring_buffer::Fixed::from(vec![0.0f32; 65536]),
                output_ring_r: ring_buffer::Fixed::from(vec![0.0f32; 65536]),
                block_counter: 0,
                stereo,
            };
        }

        Self {
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
        // ...

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
