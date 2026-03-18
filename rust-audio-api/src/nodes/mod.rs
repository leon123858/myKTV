use crate::types::AudioUnit;

pub mod delay;
pub mod file;
pub mod filter;
pub mod gain;
pub mod microphone;
pub mod mixer;
pub mod oscillator;
pub mod resampler;
pub mod convolver;

pub use delay::DelayNode;
pub use file::FileNode;
pub use filter::{FilterNode, FilterType};
pub use gain::GainNode;
pub use microphone::MicrophoneNode;
pub use mixer::MixerNode;
pub use oscillator::OscillatorNode;
pub use convolver::{ConvolverNode, ConvolverConfig};

pub enum NodeType {
    Gain(GainNode),
    Oscillator(OscillatorNode),
    Microphone(MicrophoneNode),
    File(FileNode),
    Mixer(MixerNode),
    Delay(DelayNode),
    Convolver(ConvolverNode),
    Filter(FilterNode),
}

impl NodeType {
    /// Pulls audio data.
    /// - `input`: Audio segment processed by upstream nodes
    /// - `output`: Segment to write calculated audio into
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
            NodeType::Filter(node) => node.process(input, output),
        }
    }
}
