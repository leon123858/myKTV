use crate::audio_node::node_const::{PREFERRED_SAMPLE_RATE, RING_BUFFER_CAPACITY};
use crate::audio_node::{AudioNode, AudioNodeState, AudioNodeType};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, SampleRate, Stream, StreamConfig, StreamError};
use rtrb::Producer;
use crate::audio_node::utils::{generate_input_resolve_config};

pub struct MicSrc {
    pub state: AudioNodeState,
    pub audio_producer: Option<Producer<f32>>,
    input_stream: Option<Stream>,
    device: cpal::Device,
    config: cpal::StreamConfig,
}

impl AudioNode for MicSrc {
    fn init() -> Self {
        let host = cpal::default_host();
        // 獲取預設輸入設備 (麥克風)
        let input_device = host
            .default_input_device()
            .expect("no input device available");

        println!("[HAL] Input Device: {:?}", input_device.description().unwrap().name());

        // negotiation function
        let mut resolve_config_fn = generate_input_resolve_config("Mic".parse().unwrap());

        // 協商輸入配置
        let input_config =
            resolve_config_fn(&input_device).expect("failed to resolve input device config");
        println!("[HAL] Negotiated Input Config: {:?}", input_config);

        Self {
            state: AudioNodeState::INITIALIZED,
            audio_producer: None,
            input_stream: None,
            device: input_device,
            config: input_config,
        }
    }

    fn start(&mut self) {
        // 如果 Stream 尚未建立 (第一次 Start)，則利用 Producer 建立 Stream
        if self.input_stream.is_none() {
            let mut producer = match self.audio_producer.take() {
                Some(p) => p,
                None => panic!("MicSrc: cannot start, no producer connected"),
            };

            println!(
                "[HAL] Starting Audio Producer {}, {} HZ",
                producer.slots(),
                self.config.sample_rate
            );

            while !producer.is_full() {
                let _ = producer.push(0.0);
            }

            let channels = self.config.channels;
            let stream = self
                .device
                .build_input_stream(
                    &self.config,
                    data_input_callback_creator(producer, channels),
                    err_hdl_cb,
                    None,
                )
                .expect("failed to build input stream");

            self.input_stream = Some(stream);
        }

        // 播放 (錄音)
        if let Some(stream) = &self.input_stream {
            stream.play().expect("failed to start input stream");
        }

        self.state = AudioNodeState::RUNNING;
    }

    fn stop(&mut self) {
        if let Some(stream) = &self.input_stream {
            stream.pause().expect("failed to pause input stream");
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

fn err_hdl_cb(err: StreamError) {
    eprintln!("[HAL] Input Stream Error: {}", err);
}

// 建立輸入回調閉包，這會將 Producer 所有權移入 Audio Thread
fn data_input_callback_creator(
    mut producer: Producer<f32>,
    channels: u16,
    // sample_format: SampleFormat,
    // sample_rate: i32
) -> impl FnMut(&[f32], &cpal::InputCallbackInfo) {
    move |data: &[f32], _: &cpal::InputCallbackInfo| {
        // ensure_realtime_priority();

        let gain: f32 = 100.0;
        let mut output_iter = data.iter();
        while let Some(&sample) = output_iter.next() {
            // 檢查 Producer 是否有足夠空間塞入所有聲道
            if producer.slots() >= channels as usize {
                for _ in 0..channels {
                    // 這裡建議用 push 而不是 expect，避免在音訊線程 panic
                    if let Err(_) = producer.push((sample * gain).clamp(-1.0, 1.0)) {}
                }
            }
        }
    }
}