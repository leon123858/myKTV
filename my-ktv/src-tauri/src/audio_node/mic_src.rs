use crate::audio_node::node_const::{PUSH_RING_BUFFER_CAPACITY, RESAMPLE_BUFFER_CAPACITY};
use crate::audio_node::utils::{generate_input_resolve_config, IOStreamConfig, ResamplingHandler};
use crate::audio_node::{AudioNode, AudioNodeState, AudioNodeType};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, Stream, StreamError};
use rtrb::{Consumer, Producer, RingBuffer};

pub struct MicSrc {
    pub state: AudioNodeState,
    pub audio_producer: Option<Producer<f32>>,
    pub input_producer_config: Option<IOStreamConfig>,
    input_stream: Option<Stream>,
    device: cpal::Device,
    config: IOStreamConfig,
    inner_producer: Option<Producer<f32>>,
    inner_consumer: Option<Consumer<f32>>,
}

impl AudioNode for MicSrc {
    fn init() -> Self {
        let host = cpal::default_host();
        // 獲取預設輸入設備 (麥克風)
        let input_device = host
            .default_input_device()
            .expect("no input device available");

        println!(
            "[HAL] Input Device: {:?}",
            input_device.description().unwrap().name()
        );

        // negotiation function
        let mut resolve_config_fn = generate_input_resolve_config("Mic".parse().unwrap());

        // 協商輸入配置
        let input_config =
            resolve_config_fn(&input_device).expect("failed to resolve input device config");
        println!("[HAL] Negotiated Input Config: {:?}", input_config);

        // create mic cache buffer for resample usage
        let (producer, consumer) = RingBuffer::<f32>::new(PUSH_RING_BUFFER_CAPACITY);

        Self {
            state: AudioNodeState::INITIALIZED,
            audio_producer: None,
            input_producer_config: None,
            input_stream: None,
            device: input_device,
            config: input_config,
            inner_producer: Option::from(producer),
            inner_consumer: Option::from(consumer),
        }
    }

    fn start(&mut self) {
        // 如果 Stream 尚未建立 (第一次 Start)，則利用 Producer 建立 Stream
        if self.input_stream.is_none() {
            let producer = match self.audio_producer.take() {
                Some(p) => p,
                None => panic!("MicSrc: cannot start, no producer connected"),
            };
            let producer_config = match self.input_producer_config.take() {
                Some(c) => c,
                None => panic!("MicSrc: cannot start, no producer config"),
            };
            let input_config = &self.config;

            let resampler = ResamplingHandler::new(
                producer,
                input_config.stream_config.clone(),
                producer_config.stream_config,
                self.inner_producer.take().unwrap(),
                self.inner_consumer.take().unwrap(),
                RESAMPLE_BUFFER_CAPACITY,
            );

            let stream = match input_config.sample_format {
                SampleFormat::F32 => self
                    .device
                    .build_input_stream(
                        &self.config.stream_config,
                        data_input_callback_creator::<f32>(resampler),
                        err_hdl_cb,
                        None,
                    )
                    .expect("failed to build input stream"),
                SampleFormat::I32 => self
                    .device
                    .build_input_stream(
                        &self.config.stream_config,
                        data_input_callback_creator::<i32>(resampler),
                        err_hdl_cb,
                        None,
                    )
                    .expect("failed to build input stream"),
                SampleFormat::I16 => self
                    .device
                    .build_input_stream(
                        &self.config.stream_config,
                        data_input_callback_creator::<i16>(resampler),
                        err_hdl_cb,
                        None,
                    )
                    .expect("failed to build input stream"),
                SampleFormat::U8 => self
                    .device
                    .build_input_stream(
                        &self.config.stream_config,
                        data_input_callback_creator::<u8>(resampler),
                        err_hdl_cb,
                        None,
                    )
                    .expect("failed to build input stream"),

                _ => panic!("MicSrc: cannot start, no producer sample format"),
            };

            self.input_stream = Some(stream);
        }

        // play
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

fn data_input_callback_creator<T>(
    mut handler: ResamplingHandler,
) -> impl FnMut(&[T], &cpal::InputCallbackInfo)
where
    T: Sample,
    f32: FromSample<T>,
{
    move |data: &[T], _: &cpal::InputCallbackInfo| {
        if handler.check_must_loss_all_data() {
            println!("[HAL] Producer full");
            return;
        }
        handler.process_packet(data);
    }
}
