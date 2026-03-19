use crate::types::AudioUnit;

/// A node that combines multiple audio signals into a single output.
///
/// It applies a gain factor to the mixed signal and performs hard clipping
/// to ensure the output remains within the [-1.0, 1.0] range.
///
/// Supports dynamic gain updates via [`ControlMessage::SetParameter`](crate::graph::ControlMessage::SetParameter).
///
/// # Example
/// ```no_run
/// use rust_audio_api::nodes::{MixerNode, NodeType};
/// use rust_audio_api::{AudioContext, NodeParameter};
///
/// let mut ctx = AudioContext::new().unwrap();
///
/// let mut mixer_id = None;
/// let dest_id = ctx.build_graph(|builder| {
///     let mixer = builder.add_node(NodeType::Mixer(MixerNode::with_gain(1.0)));
///     mixer_id = Some(mixer);
///     mixer
/// });
///
/// // Dynamically reduce the master mix volume
/// ctx.control_sender().send(
///     rust_audio_api::graph::ControlMessage::SetParameter(
///         mixer_id.unwrap(),
///         NodeParameter::Gain(0.5)
///     )
/// ).unwrap();
/// ```
pub struct MixerNode {
    gain: f32,
    pub clipping: bool,
}

impl Default for MixerNode {
    fn default() -> Self {
        Self::new()
    }
}

impl MixerNode {
    /// Creates a new `MixerNode` with unity gain (1.0) and clipping enabled.
    pub fn new() -> Self {
        Self { gain: 1.0, clipping: true }
    }

    /// Creates a new `MixerNode` with the specified gain factor and clipping enabled by default.
    pub fn with_gain(gain: f32) -> Self {
        Self { gain, clipping: true }
    }

    /// Sets the gain factor for the mixed output.
    pub fn set_gain(&mut self, gain: f32) {
        self.gain = gain;
    }

    /// MixerNode is a passive node that receives the aggregated `input` (the mixed result) from the graph,
    /// then applies Gain and optionally performing Clipping/Limiting to ensure the final output doesn't distort.
    #[inline(always)]
    pub fn process(&mut self, input: Option<&AudioUnit>, output: &mut AudioUnit) {
        if let Some(in_unit) = input {
            output.copy_from_slice(in_unit);

            if self.clipping {
                // Apply gain and hard clipping limit to [-1.0, 1.0] to prevent distortion
                dasp::slice::map_in_place(&mut output[..], |frame| {
                    [
                        (frame[0] * self.gain).clamp(-1.0, 1.0),
                        (frame[1] * self.gain).clamp(-1.0, 1.0),
                    ]
                });
            } else {
                // Apply gain only
                dasp::slice::map_in_place(&mut output[..], |frame| {
                    [
                        frame[0] * self.gain,
                        frame[1] * self.gain,
                    ]
                });
            }
        } else {
            // If no upstream input, output silence
            dasp::slice::equilibrium(&mut output[..]);
        }
    }
}
