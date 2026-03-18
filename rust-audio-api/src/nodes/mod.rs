use crate::types::AudioUnit;

pub mod convolver;
pub mod delay;
pub mod file;
pub mod filter;
pub mod gain;
pub mod microphone;
pub mod mixer;
pub mod oscillator;
pub mod resampler;

pub use convolver::{ConvolverConfig, ConvolverNode};
pub use delay::DelayNode;
pub use file::FileNode;
pub use filter::{FilterNode, FilterType};
pub use gain::GainNode;
pub use microphone::MicrophoneNode;
pub use mixer::MixerNode;
pub use oscillator::OscillatorNode;

/// Represents the different types of audio nodes available in the library.
///
/// Each variant wraps a specific node implementation.
pub enum NodeType {
    /// Adjusts the volume of the audio signal.
    Gain(GainNode),
    /// Generates periodic waveforms (e.g., sine waves).
    Oscillator(OscillatorNode),
    /// Captures audio from the system's default microphone.
    Microphone(MicrophoneNode),
    /// Reads audio data from a file.
    File(FileNode),
    /// Mixes multiple audio signals together.
    Mixer(MixerNode),
    /// Delays the audio signal by a specified amount of time.
    Delay(DelayNode),
    /// Applies convolution (e.g., for reverb or IR effects).
    Convolver(ConvolverNode),
    /// Applies biquad filtering (e.g., low-pass, high-pass).
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
