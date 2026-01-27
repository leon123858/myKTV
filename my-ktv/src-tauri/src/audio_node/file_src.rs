/***
 * @ Mod:       file_src
 * @ Author:    Leon Lin
 * @ Date:      20260127
 */

use crate::audio_node::{AudioNode, AudioNodeState, AudioNodeType};
use rodio::Source;
use rtrb::Producer;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::thread::JoinHandle;

pub struct FileSrc {
    pub state: AudioNodeState,
    pub audio_producer: Option<Producer<f32>>,
    keep_running: Arc<AtomicBool>,
    producer_handler: Option<JoinHandle<Producer<f32>>>,
    file_path: PathBuf,
    sample_rate: u32,
    channels: u16,
}

impl FileSrc {
    pub fn new(file_path: PathBuf, sample_rate: u32, channels: u16) -> Self {
        Self {
            state: AudioNodeState::INITIALIZED,
            audio_producer: None,
            keep_running: Arc::new(AtomicBool::new(false)),
            producer_handler: None,
            file_path,
            sample_rate,
            channels,
        }
    }
}

impl AudioNode for FileSrc {
    fn init() -> Self {
        // Default initialization - users should use FileSrc::new() instead
        Self {
            state: AudioNodeState::INITIALIZED,
            audio_producer: None,
            keep_running: Arc::new(AtomicBool::new(false)),
            producer_handler: None,
            file_path: PathBuf::from("test.mp3"),
            sample_rate: 48000,
            channels: 2,
        }
    }

    fn start(&mut self) {
        let sleep_ms = 10;
        let mut producer = match self.audio_producer.take() {
            Some(p) => p,
            None => panic!("FileSrc: cannot start audio node - no producer"),
        };

        let file_path = self.file_path.clone();
        let target_sample_rate = self.sample_rate;
        let target_channels = self.channels;
        let keep_running = Arc::clone(&self.keep_running);
        keep_running.store(true, Ordering::Relaxed);

        self.producer_handler = Some(thread::spawn(move || {
            println!("[FileSrc] Producer Thread Started");
            println!("[FileSrc] Loading file: {:?}", file_path);

            // Open the file and decode
            let file = match File::open(&file_path) {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("[FileSrc] Failed to open file: {}", e);
                    return producer;
                }
            };

            let source = match rodio::Decoder::new(BufReader::new(file)) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("[FileSrc] Failed to decode file: {}", e);
                    return producer;
                }
            };

            let source_sample_rate = source.sample_rate();
            let source_channels = source.channels();
            println!(
                "[FileSrc] Source: {}Hz, {} channels",
                source_sample_rate, source_channels
            );
            println!(
                "[FileSrc] Target: {}Hz, {} channels",
                target_sample_rate, target_channels
            );

            // Convert samples to f32 and handle resampling if needed
            let samples: Vec<f32> = source.convert_samples().collect();
            println!("[FileSrc] Loaded {} samples", samples.len());

            // Resample if needed
            let resampled_samples = if source_sample_rate != target_sample_rate {
                println!("[FileSrc] Resampling required");
                use rubato::{
                    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType,
                    WindowFunction,
                };

                let params = SincInterpolationParameters {
                    sinc_len: 256,
                    f_cutoff: 0.95,
                    interpolation: SincInterpolationType::Linear,
                    oversampling_factor: 256,
                    window: WindowFunction::BlackmanHarris2,
                };

                let mut resampler = SincFixedIn::<f32>::new(
                    target_sample_rate as f64 / source_sample_rate as f64,
                    2.0,
                    params,
                    samples.len() / source_channels as usize,
                    source_channels as usize,
                )
                .expect("Failed to create resampler");

                // Deinterleave samples
                let mut input_channels: Vec<Vec<f32>> = vec![Vec::new(); source_channels as usize];
                for (i, sample) in samples.iter().enumerate() {
                    input_channels[i % source_channels as usize].push(*sample);
                }

                // Resample each channel
                let resampled_channels = match resampler.process(&input_channels, None) {
                    Ok(output) => output,
                    Err(e) => {
                        eprintln!("[FileSrc] Resampling failed: {}", e);
                        return producer;
                    }
                };

                // Interleave resampled samples
                let mut resampled = Vec::new();
                let num_samples = resampled_channels[0].len();
                for i in 0..num_samples {
                    for channel in &resampled_channels {
                        if i < channel.len() {
                            resampled.push(channel[i]);
                        }
                    }
                }

                println!("[FileSrc] Resampled to {} samples", resampled.len());
                resampled
            } else {
                samples
            };

            // Handle channel conversion if needed
            let final_samples = if source_channels != target_channels {
                println!("[FileSrc] Channel conversion required");
                let mut converted = Vec::new();

                if source_channels == 1 && target_channels == 2 {
                    // Mono to stereo: duplicate each sample
                    for sample in &resampled_samples {
                        converted.push(*sample);
                        converted.push(*sample);
                    }
                } else if source_channels == 2 && target_channels == 1 {
                    // Stereo to mono: average pairs
                    for chunk in resampled_samples.chunks(2) {
                        let avg = chunk.iter().sum::<f32>() / chunk.len() as f32;
                        converted.push(avg);
                    }
                } else {
                    eprintln!(
                        "[FileSrc] Unsupported channel conversion: {} -> {}",
                        source_channels, target_channels
                    );
                    converted = resampled_samples;
                }

                println!("[FileSrc] Converted to {} samples", converted.len());
                converted
            } else {
                resampled_samples
            };

            // Stream samples to the producer
            println!("[FileSrc] Starting playback");
            let mut idx = 0;
            while keep_running.load(Ordering::Relaxed) && idx < final_samples.len() {
                // Try to push samples in chunks
                while producer.slots() >= 2 && idx < final_samples.len() {
                    if producer.push(final_samples[idx]).is_err() {
                        eprintln!("[FileSrc] Failed to push sample");
                        break;
                    }
                    idx += 1;
                }

                // Sleep to avoid busy-waiting
                if idx < final_samples.len() {
                    thread::sleep(std::time::Duration::from_millis(sleep_ms));
                }
            }

            println!(
                "[FileSrc] Playback finished or stopped (played {} samples)",
                idx
            );
            println!("[FileSrc] Producer Thread Stopped");
            producer
        }));

        self.state = AudioNodeState::RUNNING;
    }

    fn stop(&mut self) {
        self.keep_running.store(false, Ordering::Relaxed);

        if let Some(handle) = self.producer_handler.take() {
            if let Ok(producer) = handle.join() {
                self.audio_producer = Some(producer);
                println!("[FileSrc] Producer returned to struct");
            }
        } else {
            // Thread already finished or was never started - this is fine
            println!(
                "[FileSrc] Stop called but no active thread (already finished or never started)"
            );
        }

        self.state = AudioNodeState::STOPPED;
    }

    fn get_type(&self) -> AudioNodeType {
        AudioNodeType::SOURCE
    }

    fn get_state(&self) -> AudioNodeState {
        self.state
    }
}
