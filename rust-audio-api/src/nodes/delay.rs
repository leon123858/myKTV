use crate::types::{AudioUnit, empty_audio_unit};
use std::collections::VecDeque;

/// A node that delays the input signal by a specified number of audio blocks.
///
/// Each audio block ([`AudioUnit`]) contains [`AUDIO_UNIT_SIZE`](crate::types::AUDIO_UNIT_SIZE) samples.
///
/// Supports dynamic delay updates via [`ControlMessage::SetParameter`](crate::graph::ControlMessage::SetParameter).
///
/// # Example
/// ```no_run
/// use rust_audio_api::nodes::{DelayNode, NodeType};
/// use rust_audio_api::{AudioContext, NodeParameter};
///
/// let mut ctx = AudioContext::new().unwrap();
/// let sample_rate = ctx.sample_rate();
///
/// let mut delay_id = None;
/// let dest_id = ctx.build_graph(|builder| {
///     // Max delay 2 seconds, initial delay 0.5 seconds
///     let max_units = (sample_rate as usize * 2) / rust_audio_api::types::AUDIO_UNIT_SIZE;
///     let initial_units = (sample_rate as f32 * 0.5) as usize / rust_audio_api::types::AUDIO_UNIT_SIZE;
///     
///     let delay = builder.add_node(NodeType::Delay(DelayNode::new(max_units, initial_units)));
///     delay_id = Some(delay);
///     delay
/// });
///
/// // Dynamically change the delay to 1.0 seconds
/// let new_units = (sample_rate as f32 * 1.0) as usize / rust_audio_api::types::AUDIO_UNIT_SIZE;
/// ctx.control_sender().send(
///     rust_audio_api::graph::ControlMessage::SetParameter(
///         delay_id.unwrap(),
///         NodeParameter::DelayUnits(new_units)
///     )
/// ).unwrap();
/// ```
pub struct DelayNode {
    queue: VecDeque<AudioUnit>,
    delay_units: usize,
    max_delay_units: usize,
}

impl DelayNode {
    /// Creates a new `DelayNode`.
    ///
    /// # Parameters
    /// - `max_delay_units`: The maximum delay buffer size in units.
    /// - `default_delay_units`: The initial delay in units.
    pub fn new(max_delay_units: usize, default_delay_units: usize) -> Self {
        let delay_units = default_delay_units.min(max_delay_units);
        let mut queue = VecDeque::with_capacity(max_delay_units + 1);

        // Seed the queue with silent Units based on initial delay_units
        for _ in 0..delay_units {
            queue.push_back(empty_audio_unit());
        }

        Self {
            queue,
            delay_units,
            max_delay_units,
        }
    }

    /// Dynamically updates the delay time.
    ///
    /// If the new delay is larger than the current one, silent blocks are inserted.
    /// If it is smaller, old blocks are discarded.
    pub fn set_delay_units(&mut self, units: usize) {
        let target_units = units.min(self.max_delay_units);

        if target_units > self.delay_units {
            // Increase delay: add silent Units
            for _ in 0..(target_units - self.delay_units) {
                self.queue.push_front(empty_audio_unit()); // Push to front; these are the new delayed units
            }
        } else if target_units < self.delay_units {
            // Decrease delay: discard old Units (front represents the oldest)
            for _ in 0..(self.delay_units - target_units) {
                self.queue.pop_front();
            }
        }
        self.delay_units = target_units;
    }

    #[inline(always)]
    pub fn process(&mut self, input: Option<&AudioUnit>, output: &mut AudioUnit) {
        // Core algorithm: push input (or silence) into the queue
        if let Some(in_unit) = input {
            self.queue.push_back(*in_unit);
        } else {
            self.queue.push_back(empty_audio_unit());
        }

        // Then pop a unit from the queue as current output
        // If delay_units is 0, the queue will only contain the unit just pushed;
        // popping it results in zero delay.
        if let Some(delayed_unit) = self.queue.pop_front() {
            output.copy_from_slice(&delayed_unit);
        } else {
            // Fallback mechanism; theoretically, the queue should always have at least one unit
            dasp::slice::equilibrium(&mut output[..]);
        }
    }
}
