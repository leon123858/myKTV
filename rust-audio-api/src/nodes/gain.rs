use crate::types::AudioUnit;

/// A node that adjusts the volume (gain) of the input signal.
///
/// Supports dynamic gain updates via [`ControlMessage::SetParameter`](crate::graph::ControlMessage::SetParameter).
///
/// # Example
/// ```no_run
/// use rust_audio_api::nodes::{GainNode, NodeType};
/// use rust_audio_api::{AudioContext, NodeParameter};
///
/// let mut ctx = AudioContext::new().unwrap();
///
/// let mut gain_id = None;
/// let dest_id = ctx.build_graph(|builder| {
///     let gain = builder.add_node(NodeType::Gain(GainNode::new(0.5)));
///     gain_id = Some(gain);
///     gain
/// });
///
/// // Dynamically change the gain to 1.0 (full volume)
/// ctx.control_sender().send(
///     rust_audio_api::graph::ControlMessage::SetParameter(
///         gain_id.unwrap(),
///         NodeParameter::Gain(1.0)
///     )
/// ).unwrap();
/// ```
pub struct GainNode {
    gain: f32,
}

impl GainNode {
    /// Creates a new `GainNode` with the specified gain factor.
    ///
    /// # Parameters
    /// - `gain`: The multiplier for the audio signal (e.g., 1.0 for unity gain, 0.5 for half volume).
    pub fn new(gain: f32) -> Self {
        Self { gain }
    }

    /// Sets the gain factor for this node.
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
