use crate::types::AudioUnit;

pub struct MixerNode {
    gain: f32,
}

impl MixerNode {
    pub fn new() -> Self {
        Self { gain: 1.0 }
    }

    pub fn with_gain(gain: f32) -> Self {
        Self { gain }
    }

    pub fn set_gain(&mut self, gain: f32) {
        self.gain = gain;
    }

    /// MixerNode is a passive node that receives the aggregated `input` (the mixed result) from the graph,
    /// then applies Gain and performing Clipping/Limiting to ensure the final output doesn't distort.
    #[inline(always)]
    pub fn process(&mut self, input: Option<&AudioUnit>, output: &mut AudioUnit) {
        if let Some(in_unit) = input {
            output.copy_from_slice(in_unit);

            // Apply gain and hard clipping limit to [-1.0, 1.0] to prevent distortion
            dasp::slice::map_in_place(&mut output[..], |frame| {
                [
                    (frame[0] * self.gain).clamp(-1.0, 1.0),
                    (frame[1] * self.gain).clamp(-1.0, 1.0),
                ]
            });
        } else {
            // If no upstream input, output silence
            dasp::slice::equilibrium(&mut output[..]);
        }
    }
}
