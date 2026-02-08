/***
 * @ Mod:       file_src
 * @ Author:    Leon Lin
 * @ Date:      20260127
 */

use crate::audio_node::node_const::{
    RESAMPLE_BUFFER_CAPACITY, RESAMPLE_INNER_CACHE_BUFFER_CAPACITY,
};
use crate::audio_node::utils::ResamplingHandler;
use crate::audio_node::{AudioNode, AudioNodeState, AudioNodeType};
use cpal::{BufferSize, ChannelCount, StreamConfig};
use rodio::Source;
use rtrb::{Producer, RingBuffer};
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
    file_path: Option<PathBuf>,
    producer_sample_rate: Option<u32>,
    producer_channels: Option<usize>,
    sleep_ms: u64,
}

impl FileSrc {
    pub fn set_config(&mut self, file_path: PathBuf, sample_rate: u32, channels: usize) {
        self.file_path = Some(file_path);
        self.producer_sample_rate = Some(sample_rate);
        self.producer_channels = Some(channels);
    }
}

impl AudioNode for FileSrc {
    fn init() -> Self {
        // Default initialization
        Self {
            state: AudioNodeState::INITIALIZED,
            audio_producer: None,
            keep_running: Arc::new(AtomicBool::new(false)),
            producer_handler: None,
            file_path: None,
            producer_sample_rate: None,
            producer_channels: None,
            sleep_ms: 10,
        }
    }

    fn start(&mut self) {
        let sleep_ms = self.sleep_ms;
        let producer = match self.audio_producer.take() {
            Some(p) => p,
            None => panic!("FileSrc: cannot start audio node - no producer"),
        };
        let file_path = match self.file_path.take() {
            Some(p) => p,
            None => panic!("FileSrc: cannot start audio node - no file path"),
        };
        let target_sample_rate = match self.producer_sample_rate.take() {
            Some(p) => p,
            None => panic!("FileSrc: cannot start audio node - no file path"),
        };
        let target_channels = match self.producer_channels.take() {
            Some(p) => p,
            None => panic!("FileSrc: cannot start audio node - no file path"),
        };

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

            let mut source = match rodio::Decoder::new(BufReader::new(file)) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("[FileSrc] Failed to decode file: {}", e);
                    return producer;
                }
            };

            let source_sample_rate = source.sample_rate();
            let source_channels = source.channels() as usize;
            println!(
                "[FileSrc] Source: {}Hz, {} channels",
                source_sample_rate, source_channels
            );
            println!(
                "[FileSrc] Target: {}Hz, {} channels",
                target_sample_rate, target_channels
            );

            let src_cfg = StreamConfig {
                channels: source_channels as ChannelCount,
                sample_rate: source_sample_rate,
                buffer_size: BufferSize::Default,
            };
            let trg_cfg = StreamConfig {
                channels: target_channels as ChannelCount,
                sample_rate: target_sample_rate,
                buffer_size: BufferSize::Default,
            };
            let chunk_size = RESAMPLE_BUFFER_CAPACITY * source_channels;
            let (resample_producer, resample_consumer) =
                RingBuffer::<f32>::new(RESAMPLE_INNER_CACHE_BUFFER_CAPACITY);
            let mut resampler = ResamplingHandler::new(
                producer,
                src_cfg,
                trg_cfg,
                resample_producer,
                resample_consumer,
                chunk_size,
            );
            let mut data_buffer = vec![0; chunk_size];
            // Convert samples to f32 and handle resampling if needed
            let mut is_end = false;
            while keep_running.load(Ordering::Relaxed) {
                if is_end {
                    break;
                }
                while !resampler.check_must_no_loss_data(chunk_size) {
                    thread::sleep(std::time::Duration::from_millis(sleep_ms));
                }
                for cur_chunk_size in 0..chunk_size {
                    let sample_option = source.next();
                    if let Some(sample) = sample_option {
                        data_buffer[cur_chunk_size] = sample;
                    } else {
                        is_end = true;
                        data_buffer[cur_chunk_size] = 0;
                    }
                }
                resampler.process_packet(&data_buffer);
            }
            resampler.producer
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
