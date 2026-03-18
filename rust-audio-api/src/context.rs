use crate::graph::{ControlMessage, GraphBuilder, NodeId};
use crate::types::{AUDIO_UNIT_SIZE, AudioUnit};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream, StreamConfig};
use crossbeam_channel::{Sender, unbounded};
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, AtomicU32, Ordering};
use std::time::Instant;

/// Monitor for audio thread performance.
///
/// It tracks the number of late callbacks and the current CPU load percentage
/// of the audio processing thread.
#[derive(Clone)]
pub struct PerformanceMonitor {
    /// Number of times the audio thread failed to meet the real-time deadline.
    pub late_callbacks: Arc<AtomicU32>,
    /// Current CPU load of the audio processing thread in percentage (0-100).
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

/// The main entry point for the audio system.
///
/// `AudioContext` manages the audio graph, the audio backend (CPAL),
/// and the real-time audio thread. It provides a high-level API for
/// building and controlling audio processing graphs.
///
/// # Examples
///
/// ### Basic Usage
/// ```no_run
/// use rust_audio_api::AudioContext;
///
/// let mut ctx = AudioContext::new().unwrap();
/// // ... build graph ...
/// // ctx.resume(destination_id).unwrap();
/// ```
///
/// ### Dynamic Parameter Updates
/// ```no_run
/// use rust_audio_api::{AudioContext, NodeParameter};
/// use rust_audio_api::nodes::{GainNode, NodeType};
///
/// let mut ctx = AudioContext::new().unwrap();
/// let mut gain_id = None;
///
/// let dest_id = ctx.build_graph(|builder| {
///     let gain = builder.add_node(NodeType::Gain(GainNode::new(0.5)));
///     gain_id = Some(gain);
///     gain
/// });
///
/// ctx.resume(dest_id).unwrap();
///
/// // Later, send a message to change the gain
/// let sender = ctx.control_sender();
/// sender.send(rust_audio_api::graph::ControlMessage::SetParameter(
///     gain_id.unwrap(),
///     NodeParameter::Gain(0.8)
/// )).unwrap();
/// ```
pub struct AudioContext {
    stream: Option<Stream>,
    sample_rate: u32,
    msg_sender: Sender<ControlMessage>,
    graph_builder: Option<GraphBuilder>,
    performance_monitor: PerformanceMonitor,
}

impl AudioContext {
    /// Creates a new `AudioContext` with the default output device and sample rate.
    pub fn new() -> Result<Self, anyhow::Error> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .expect("Default output device not found");
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

    /// Returns a `PerformanceMonitor` to track the audio thread's health.
    pub fn performance_monitor(&self) -> PerformanceMonitor {
        self.performance_monitor.clone()
    }

    /// Returns the sample rate of the audio context.
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Provides a [`GraphBuilder`] to construct the audio processing graph.
    ///
    /// This method takes a closure where you can add nodes and define their connections.
    /// It returns the [`NodeId`] of the final destination node in the graph.
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

    /// Starts the audio processing thread and begins playback.
    ///
    /// This method finalizes the graph construction and hands it over to the audio backend.
    /// `destination_id` should be the ID of the final node that outputs audio.
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
                        for f in frame.iter_mut().take(channels).skip(2) {
                            *f = T::from_sample(0.0);
                        }
                    } else if channels == 1 {
                        let mono = (sample_f32[0] + sample_f32[1]) * 0.5;
                        frame[0] = T::from_sample(mono);
                    }
                }

                let elapsed_micros = start_time.elapsed().as_micros();
                let max_allowed_micros =
                    (frame_count as f64 / sample_rate as f64 * 1_000_000.0) as u128;

                let load_percent =
                    ((elapsed_micros as f64 / max_allowed_micros as f64) * 100.0) as u8;
                monitor
                    .current_load_percent
                    .store(load_percent, Ordering::Relaxed);

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
