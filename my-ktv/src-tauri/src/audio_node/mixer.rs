/***
 * @ Mod:       mixer
 * @ Author:    Leon Lin
 * @ Date:      20260128
 */

use crate::audio_node::node_const::RING_BUFFER_CAPACITY;
use crate::audio_node::{AudioNode, AudioNodeState, AudioNodeType};
use rtrb::{Consumer, Producer, RingBuffer};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::thread::JoinHandle;

pub struct Mixer {
    pub state: AudioNodeState,
    pub audio_producer: Option<Producer<f32>>,
    input_consumers: Arc<Mutex<Vec<Consumer<f32>>>>,
    pub input_producers: Vec<Producer<f32>>,
    keep_running: Arc<AtomicBool>,
    mixer_thread: Option<JoinHandle<Producer<f32>>>,
}

impl Mixer {
    /// Create a mixer with a specified number of input channels
    pub fn new(num_inputs: usize) -> Self {
        let mut input_producers = Vec::new();
        let mut input_consumers = Vec::new();

        // Create ring buffers for each input
        for _ in 0..num_inputs {
            let (producer, consumer) = RingBuffer::<f32>::new(RING_BUFFER_CAPACITY);
            input_producers.push(producer);
            input_consumers.push(consumer);
        }

        Self {
            state: AudioNodeState::INITIALIZED,
            audio_producer: None,
            input_consumers: Arc::new(Mutex::new(input_consumers)),
            input_producers,
            keep_running: Arc::new(AtomicBool::new(false)),
            mixer_thread: None,
        }
    }

    /// Add a new input channel dynamically
    pub fn add_input(&mut self) -> Producer<f32> {
        let (producer, consumer) = RingBuffer::<f32>::new(RING_BUFFER_CAPACITY);

        let mut consumers = self.input_consumers.lock().unwrap();
        consumers.push(consumer);

        producer
    }

    /// Get a producer for a specific input slot
    pub fn take_input_producer(&mut self, index: usize) -> Option<Producer<f32>> {
        if index < self.input_producers.len() {
            Some(self.input_producers.remove(index))
        } else {
            None
        }
    }
}

impl AudioNode for Mixer {
    fn init() -> Self {
        Mixer::new(0)
    }

    fn start(&mut self) {
        let mut output_producer = match self.audio_producer.take() {
            Some(p) => p,
            None => panic!("Mixer: cannot start - no output producer"),
        };

        let input_consumers = Arc::clone(&self.input_consumers);
        let keep_running = Arc::clone(&self.keep_running);
        keep_running.store(true, Ordering::Relaxed);

        self.mixer_thread = Some(thread::spawn(move || {
            println!("[Mixer] Mixer Thread Started");

            while keep_running.load(Ordering::Relaxed) {
                let mut consumers = input_consumers.lock().unwrap();

                // Process in very small chunks for low latency
                let chunk_size = 64;

                // Check if we have space in output
                if output_producer.slots() < chunk_size {
                    drop(consumers);
                    continue;
                }

                // Find maximum available samples (process as much as we can)
                let max_available = consumers.iter().map(|c| c.slots()).max().unwrap_or(0);

                if max_available == 0 {
                    drop(consumers);
                    continue;
                }

                let samples_to_process = max_available.min(chunk_size);

                // Mix samples from all inputs
                // CRITICAL CHANGE: Process each input independently
                let mut mixed_samples = vec![0.0f32; samples_to_process];
                let mut active_inputs_count = 0;

                for consumer in consumers.iter_mut() {
                    let available = consumer.slots();
                    if available == 0 {
                        continue; // Skip inputs with no data
                    }

                    let to_read = available.min(samples_to_process);

                    match consumer.read_chunk(to_read) {
                        Ok(chunk) => {
                            let (first, second) = chunk.as_slices();

                            // Add samples from this input
                            for (i, &sample) in first.iter().enumerate() {
                                mixed_samples[i] += sample;
                            }
                            for (i, &sample) in second.iter().enumerate() {
                                mixed_samples[first.len() + i] += sample;
                            }

                            chunk.commit_all();
                            active_inputs_count += 1;
                        }
                        Err(_) => {
                            // If we can't read from this input, skip it
                            continue;
                        }
                    }
                }

                // Normalize by number of ACTIVE inputs to prevent clipping
                if active_inputs_count > 0 {
                    let scale = 1.0 / active_inputs_count as f32;
                    for sample in mixed_samples.iter_mut() {
                        *sample *= scale;
                        // Soft clipping
                        *sample = sample.max(-1.0).min(1.0);
                    }
                }

                // Write mixed samples to output
                for sample in mixed_samples {
                    if output_producer.push(sample).is_err() {
                        eprintln!("[Mixer] Failed to push sample to output");
                        break;
                    }
                }

                drop(consumers);

                // No sleep - process as fast as possible for lowest latency
            }

            println!("[Mixer] Mixer Thread Stopped");
            output_producer
        }));

        self.state = AudioNodeState::RUNNING;
    }

    fn stop(&mut self) {
        self.keep_running.store(false, Ordering::Relaxed);

        if let Some(handle) = self.mixer_thread.take() {
            if let Ok(producer) = handle.join() {
                self.audio_producer = Some(producer);
                println!("[Mixer] Producer returned to struct");
            }
        }

        self.state = AudioNodeState::STOPPED;
    }

    fn get_type(&self) -> AudioNodeType {
        AudioNodeType::MIXER
    }

    fn get_state(&self) -> AudioNodeState {
        self.state
    }
}
