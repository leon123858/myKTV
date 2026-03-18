use crate::types::AudioUnit;

pub struct GainNode {
    gain: f32,
}

impl GainNode {
    pub fn new(gain: f32) -> Self {
        Self { gain }
    }

    pub fn set_gain(&mut self, gain: f32) {
        self.gain = gain;
    }

    /// GainNode is a passive node; it requires input to function.
    /// It doesn't manage a ringbuf; it simply applies gain to each incoming frame.
    #[inline(always)]
    pub fn process(&mut self, input: Option<&AudioUnit>, output: &mut AudioUnit) {
        if let Some(in_unit) = input {
            output.copy_from_slice(in_unit);
            dasp::slice::map_in_place(&mut output[..], |f| [f[0] * self.gain, f[1] * self.gain]);
        } else {
            // If no upstream input, output silence
            dasp::slice::equilibrium(&mut output[..]);
        }
    }
}
