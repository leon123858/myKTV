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

    /// Mixer Node 是一個被動節點，接收 Graph 已經加總好的 `input` (混音結果)，
    /// 接著套用 Gain，並進行 Clipping/Limiting，確保最終輸出不破音。
    #[inline(always)]
    pub fn process(&mut self, input: Option<&AudioUnit>, output: &mut AudioUnit) {
        if let Some(in_unit) = input {
            for i in 0..in_unit.len() {
                // Apply gain
                let l = in_unit[i][0] * self.gain;
                let r = in_unit[i][1] * self.gain;

                // Hard clipping limit to [-1.0, 1.0] to prevent distortion
                output[i][0] = l.clamp(-1.0, 1.0);
                output[i][1] = r.clamp(-1.0, 1.0);
            }
        } else {
            // 如果沒有上游輸入，就輸出靜音
            for i in 0..output.len() {
                output[i][0] = 0.0;
                output[i][1] = 0.0;
            }
        }
    }
}
