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
        
        // 依照初始設定的 delay_units 塞入相對應數量的靜音 Unit
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
            // 需要增加 delay，補上靜音 Unit
            for _ in 0..(target_units - self.delay_units) {
                self.queue.push_front(empty_audio_unit()); // 推到最前面，代表這些是剛進來要被 delay 的
            }
        } else if target_units < self.delay_units {
            // 需要減少 delay，直接丟棄舊的 Unit (前面的代表最老的)
            for _ in 0..(self.delay_units - target_units) {
                self.queue.pop_front();
            }
        }
        self.delay_units = target_units;
    }

    #[inline(always)]
    pub fn process(&mut self, input: Option<&AudioUnit>, output: &mut AudioUnit) {
        // 核心演算法: 先將 input (或靜音) push 進 queue
        if let Some(in_unit) = input {
            self.queue.push_back(*in_unit);
        } else {
            self.queue.push_back(empty_audio_unit());
        }

        // 然後從 queue pop 出一個 unit 作為 current output
        // 若 delay_units 為 0，則 queue 裡面只會有剛放進去的一塊，pop 出來就等於完全沒 delay
        if let Some(delayed_unit) = self.queue.pop_front() {
            output.copy_from_slice(&delayed_unit);
        } else {
            // 防呆機制，理論上 queue 至少會有一塊剛被 push 進去的
            dasp::slice::equilibrium(&mut output[..]); 
        }
    }
}
