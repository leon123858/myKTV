use crate::node::AudioNode;
use dasp::signal::{self, Signal};

pub struct OscillatorNode {
    // dasp 提供的訊號產生器
    signal: Box<dyn Signal<Frame = f64> + Send>,
    gain: f32,
}

impl OscillatorNode {
    pub fn new(sample_rate: f64, frequency: f64) -> Self {
        // 使用 dasp 建立一個正弦波
        let sig = signal::rate(sample_rate).const_hz(frequency).sine();
        Self {
            signal: Box::new(sig),
            gain: 1.0, // 預設音量
        }
    }

    pub fn set_gain(&mut self, gain: f32) {
        self.gain = gain;
    }
}

impl AudioNode for OscillatorNode {
    fn process(&mut self) -> [f32; 2] {
        // 取得下一個採樣點並套用 gain
        let sample = (self.signal.next() as f32) * self.gain;
        // 輸出雙聲道 (Mono 轉 Stereo)
        [sample, sample]
    }
}
