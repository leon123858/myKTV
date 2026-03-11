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

    /// Gain Node 是一個被動節點，需要看 input 才能運作。
    /// 它不會去管理 ringbuf，單純對每一個傳進來的 frame 套用 gain 乘積。
    #[inline(always)]
    pub fn process(&mut self, input: Option<&AudioUnit>, output: &mut AudioUnit) {
        if let Some(in_unit) = input {
            output.copy_from_slice(in_unit);
            dasp::slice::map_in_place(&mut output[..], |f| [f[0] * self.gain, f[1] * self.gain]);
        } else {
            // 如果沒有上游輸入，就輸出靜音
            dasp::slice::equilibrium(&mut output[..]);
        }
    }
}
