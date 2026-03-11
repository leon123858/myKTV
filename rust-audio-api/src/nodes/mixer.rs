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
            output.copy_from_slice(in_unit);

            // Apply gain and hard clipping limit to [-1.0, 1.0] to prevent distortion
            dasp::slice::map_in_place(&mut output[..], |frame| {
                [
                    (frame[0] * self.gain).clamp(-1.0, 1.0),
                    (frame[1] * self.gain).clamp(-1.0, 1.0),
                ]
            });
        } else {
            // 如果沒有上游輸入，就輸出靜音
            dasp::slice::equilibrium(&mut output[..]);
        }
    }
}
