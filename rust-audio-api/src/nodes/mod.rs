use crate::types::AudioUnit;

pub mod file;
pub mod gain;
pub mod microphone;
pub mod oscillator;

pub use file::FileNode;
pub use gain::GainNode;
pub use microphone::MicrophoneNode;
pub use oscillator::OscillatorNode;

pub enum NodeType {
    Gain(GainNode),
    Oscillator(OscillatorNode),
    Microphone(MicrophoneNode),
    File(FileNode),
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
        }
    }
}
