use crate::types::AudioUnit;

pub mod delay;
pub mod file;
pub mod gain;
pub mod microphone;
pub mod mixer;
pub mod oscillator;
pub mod resampler;
pub mod convolver;

pub use delay::DelayNode;
pub use file::FileNode;
pub use gain::GainNode;
pub use microphone::MicrophoneNode;
pub use mixer::MixerNode;
pub use oscillator::OscillatorNode;
pub use convolver::ConvolverNode;

pub enum NodeType {
    Gain(GainNode),
    Oscillator(OscillatorNode),
    Microphone(MicrophoneNode),
    File(FileNode),
    Mixer(MixerNode),
    Delay(DelayNode),
    Convolver(ConvolverNode),
}

impl NodeType {
    /// 拉取 (Pull) 音訊資料。
    /// - `input`: 上游節點處理完的音訊片段 (例如 Gain 需要先拿別人的聲音來放大)
    /// - `output`: 負責把計算完的音訊寫入這個陣列中
    #[inline(always)]
    pub fn process(&mut self, input: Option<&AudioUnit>, output: &mut AudioUnit) {
        match self {
            NodeType::Gain(node) => node.process(input, output),
            NodeType::Oscillator(node) => node.process(input, output),
            NodeType::Microphone(node) => node.process(input, output),
            NodeType::File(node) => node.process(input, output),
            NodeType::Mixer(node) => node.process(input, output),
            NodeType::Delay(node) => node.process(input, output),
            NodeType::Convolver(node) => node.process(input, output),
        }
    }
}
