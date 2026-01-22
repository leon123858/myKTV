use crate::audio_node::node_const::{
    MOCK_AUDIO_SAMPLE_HZ, MOCK_AUDIO_SAMPLE_RATE, PREFERRED_SAMPLE_RATE,
};
use crate::audio_node::{AudioNode, AudioNodeState, AudioNodeType};
use rtrb::Producer;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::thread::JoinHandle;

pub struct FakeAudioWaveSRC {
    pub state: AudioNodeState,
    phase: f32,
    pub audio_producer: Option<Producer<f32>>,
    keep_running: Arc<AtomicBool>,
    producer_handler: Option<JoinHandle<(Producer<f32>, f32)>>,
}

impl AudioNode for FakeAudioWaveSRC {
    fn init() -> Self {
        Self {
            state: AudioNodeState::INITIALIZED,
            phase: 0.0,
            audio_producer: None,
            keep_running: Arc::new(AtomicBool::new(false)),
            producer_handler: None,
        }
    }
    fn start(&mut self) {
        let sleep_ms = 10;
        let mut producer = match self.audio_producer.take() {
            Some(p) => p,
            None => panic!("FakeAudioWaveSRC: cannot start audio node"),
        };

        let mut phase = self.phase;
        let keep_running = Arc::clone(&self.keep_running);
        keep_running.store(true, Ordering::Relaxed);

        self.producer_handler = Some(thread::spawn(move || {
            println!("Producer Thread Started");

            while keep_running.load(Ordering::Relaxed) {
                match generate_sine_wave(&mut producer, phase) {
                    Ok(new_phase) => {
                        phase = new_phase;
                    }
                    Err(e) => {
                        println!("{}", e);
                        break;
                    }
                }
                // 避免空轉佔用 CPU，當 Buffer 滿時休息一下
                thread::sleep(std::time::Duration::from_millis(sleep_ms));
            }

            println!("Producer Thread Stopped");
            (producer, phase)
        }));

        self.state = AudioNodeState::RUNNING;
    }

    fn stop(&mut self) {
        self.keep_running.store(false, Ordering::Relaxed);

        if let Some(handle) = self.producer_handler.take() {
            if let Ok((producer, last_phase)) = handle.join() {
                // store back state
                self.audio_producer = Some(producer);
                self.phase = last_phase;
                println!("Producer and Phase returned to struct");
            }
        } else {
            panic!("FakeAudioWaveSRC: cannot stop audio node");
        }

        self.state = AudioNodeState::STOPPED;
    }
    fn get_type(&self) -> AudioNodeType {
        AudioNodeType::SOURCE
    }
    fn get_state(&self) -> AudioNodeState {
        self.state.clone()
    }
}

fn generate_sine_wave(producer: &mut Producer<f32>, phase: f32) -> Result<f32, String> {
    let sample_rate = MOCK_AUDIO_SAMPLE_RATE;
    let frequency = MOCK_AUDIO_SAMPLE_HZ;
    let mut cur_phase = phase;

    while producer.slots() >= 2 {
        for _ in 0..2 {
            let val = (cur_phase * 2.0 * std::f32::consts::PI).sin() * 0.1;
            if producer.push(val).is_err() {
                return Err("FakeAudioWaveSRC: push error".into());
            }
            cur_phase = (cur_phase + frequency / sample_rate) % 1.0;
        }
    }
    Ok(cur_phase)
}
