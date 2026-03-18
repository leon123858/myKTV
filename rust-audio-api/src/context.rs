use crate::graph::{ControlMessage, GraphBuilder, NodeId};
use crate::types::{AUDIO_UNIT_SIZE, AudioUnit};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream, StreamConfig};
use crossbeam_channel::{Sender, unbounded};
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, AtomicU32, Ordering};
use std::time::Instant;

#[derive(Clone)]
pub struct PerformanceMonitor {
    pub late_callbacks: Arc<AtomicU32>,
    pub current_load_percent: Arc<AtomicU8>,
}

impl Default for PerformanceMonitor {
    fn default() -> Self {
        Self {
            late_callbacks: Arc::new(AtomicU32::new(0)),
            current_load_percent: Arc::new(AtomicU8::new(0)),
        }
    }
}

pub struct AudioContext {
    stream: Option<Stream>,
    sample_rate: u32,
    msg_sender: Sender<ControlMessage>,
    graph_builder: Option<GraphBuilder>,
    performance_monitor: PerformanceMonitor,
}

impl AudioContext {
    pub fn new() -> Result<Self, anyhow::Error> {
        let host = cpal::default_host();
        let device = host.default_output_device().expect("Default output device not found");
        let supported_config = device.default_output_config()?;
        let sample_rate = supported_config.sample_rate();

        let (tx, _rx) = unbounded();

        Ok(Self {
            stream: None,
            sample_rate,
            msg_sender: tx,
            graph_builder: Some(GraphBuilder::new()),
            performance_monitor: PerformanceMonitor::default(),
        })
    }

    pub fn performance_monitor(&self) -> PerformanceMonitor {
        self.performance_monitor.clone()
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Provides GraphBuilder for user to construct a static graph
    pub fn build_graph<F>(&mut self, builder_func: F) -> NodeId
    where
        F: FnOnce(&mut GraphBuilder) -> NodeId,
    {
        if let Some(mut gb) = self.graph_builder.take() {
            let dest_id = builder_func(&mut gb);
            self.graph_builder = Some(gb);
            dest_id
        } else {
            panic!("GraphBuilder already consumed, cannot rebuild topology");
        }
    }

    /// Starts audio playback (generates StaticGraph and hands it to CPAL audio thread)
    pub fn resume(&mut self, destination_id: NodeId) -> Result<(), anyhow::Error> {
        if self.stream.is_some() {
            return Ok(());
        }

        let host = cpal::default_host();
        let device = host.default_output_device().unwrap();
        let supported_config = device.default_output_config()?;
        let sample_format = supported_config.sample_format();
        let config: StreamConfig = supported_config.into();

        // Take GraphBuilder and generate StaticGraph
        let builder = self.graph_builder.take().expect("GraphBuilder is missing");
        let (tx, rx) = unbounded();
        self.msg_sender = tx; // Update control sender held by the main thread

        let static_graph = builder.build(destination_id, rx);

        let stream = match sample_format {
            SampleFormat::F32 => self.build_stream::<f32>(&device, &config, static_graph)?,
            SampleFormat::I16 => self.build_stream::<i16>(&device, &config, static_graph)?,
            SampleFormat::U16 => self.build_stream::<u16>(&device, &config, static_graph)?,
            _ => return Err(anyhow::anyhow!("Unsupported audio output device format")),
        };

        stream.play()?;
        self.stream = Some(stream);
        Ok(())
    }

    fn build_stream<T>(
        &self,
        device: &cpal::Device,
        config: &StreamConfig,
        mut graph: crate::graph::StaticGraph,
    ) -> Result<Stream, anyhow::Error>
    where
        T: cpal::Sample + cpal::SizedSample + cpal::FromSample<f32>,
    {
        let channels = config.channels as usize;
        let sample_rate = self.sample_rate;
        let monitor = self.performance_monitor.clone();

        let mut unit_frame_index = AUDIO_UNIT_SIZE;
        let mut current_unit: AudioUnit = [[0.0; 2]; AUDIO_UNIT_SIZE];

        let stream = device.build_output_stream(
            config,
            move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                let start_time = Instant::now();
                let frame_count = data.len() / channels;

                for frame in data.chunks_mut(channels) {
                    if unit_frame_index >= AUDIO_UNIT_SIZE {
                        let new_unit = graph.pull_next_unit();
                        current_unit.copy_from_slice(new_unit);
                        unit_frame_index = 0;
                    }

                    let sample_f32 = current_unit[unit_frame_index];
                    unit_frame_index += 1;

                    // Format conversion to T (f32, i16, u16) in CPAL buffers & downmix/upmix handling
                    if channels >= 2 {
                        frame[0] = T::from_sample(sample_f32[0]);
                        frame[1] = T::from_sample(sample_f32[1]);
                        for i in 2..channels {
                            frame[i] = T::from_sample(0.0);
                        }
                    } else if channels == 1 {
                        let mono = (sample_f32[0] + sample_f32[1]) * 0.5;
                        frame[0] = T::from_sample(mono);
                    }
                }

                let elapsed_micros = start_time.elapsed().as_micros();
                let max_allowed_micros = (frame_count as f64 / sample_rate as f64 * 1_000_000.0) as u128;
                
                let load_percent = ((elapsed_micros as f64 / max_allowed_micros as f64) * 100.0) as u8;
                monitor.current_load_percent.store(load_percent, Ordering::Relaxed);
                
                if elapsed_micros > max_allowed_micros {
                    monitor.late_callbacks.fetch_add(1, Ordering::Relaxed);
                }
            },
            |err| eprintln!("Audio stream error: {}", err),
            None,
        )?;

        Ok(stream)
    }

    /// Returns a Sender for sending control messages (non-blocking)
    pub fn control_sender(&self) -> Sender<ControlMessage> {
        self.msg_sender.clone()
    }
}
