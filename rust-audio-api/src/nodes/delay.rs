use crate::types::{empty_audio_unit, AudioUnit};
use std::collections::VecDeque;

pub struct DelayNode {
    queue: VecDeque<AudioUnit>,
    delay_units: usize,
    max_delay_units: usize,
}

impl DelayNode {
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
