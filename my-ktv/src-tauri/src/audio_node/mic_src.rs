use crate::audio_node::utils::{generate_input_resolve_config, IOStreamConfig};
use crate::audio_node::{AudioNode, AudioNodeState, AudioNodeType};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, Stream, StreamConfig, StreamError};
use rtrb::{Consumer, Producer, RingBuffer};
use rubato::{Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction};
use crate::audio_node::node_const::{RESAMPLE_BUFFER_CAPACITY, PUSH_RING_BUFFER_CAPACITY};

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

            let resampler = ResamplingHandler::new(producer,
                                                   input_config.stream_config.clone(),
                                                   producer_config.stream_config,
                                                   self.inner_producer.take().unwrap(),
                                                   self.inner_consumer.take().unwrap(),
                                                   RESAMPLE_BUFFER_CAPACITY);

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
        if handler.check_producer_full() {
            println!("[HAL] Producer full");
            return;
        }
        handler.process_packet(data);
    }
}

pub struct ResamplingHandler {
    resampler: SincFixedIn<f32>,
    input_channels: Vec<Vec<f32>>,
    output_channels: Vec<Vec<f32>>,
    src_channels_cnt: usize,
    target_channels_cnt: usize,
    producer: Producer<f32>,
    inner_producer: Producer<f32>,
    inner_consumer: Consumer<f32>,
}

impl ResamplingHandler {
    pub fn new(producer: Producer<f32>,
               src_cfg: StreamConfig,
               target_cfg: StreamConfig,
               inner_producer: Producer<f32>,
               inner_consumer: Consumer<f32>,
               max_frames: usize) -> Self {
        let params = SincInterpolationParameters {
            sinc_len: 256,
            f_cutoff: 0.95,
            interpolation: SincInterpolationType::Linear,
            oversampling_factor: 256,
            window: WindowFunction::BlackmanHarris2,
        };

        if inner_producer.slots() < max_frames {
            panic!("mid buf should bigger than max frame");
        }

        let src_channels = src_cfg.channels as usize;
        let target_channels = target_cfg.channels as usize;

        let src_sample_rate = src_cfg.sample_rate;
        let target_sample_rate = target_cfg.sample_rate;

        // 預估輸出緩衝區大小（加上安全邊際）
        let ratio = (target_sample_rate / src_sample_rate) as f64;

        let resampler = SincFixedIn::<f32>::new(
            ratio,
            2.0,
            params,
            max_frames,
            src_channels,
        ).unwrap();
        let input_channels = resampler.input_buffer_allocate(true);
        let output_channels = resampler.output_buffer_allocate(true);

        Self {
            resampler,
            input_channels,
            output_channels,
            src_channels_cnt: src_channels,
            target_channels_cnt: target_channels,
            producer,
            inner_producer,
            inner_consumer,
        }
    }

    pub fn process_packet<T>(&mut self, input_data: &[T])
    where
        T: Sample,
        f32: FromSample<T>, {
        // push data into resample buffer
        let should_push_data_cnt = input_data.len().min(self.inner_producer.slots());
        if should_push_data_cnt == 0 {
            println!("[HAL] should_push_data_cnt is zero");
            return;
        }
        match self.inner_producer.write_chunk(should_push_data_cnt) {
            Ok(mut chunk) => {
                let (first, second) = chunk.as_mut_slices();
                let mid = first.len();
                let mut cursor = 0;
                for i in 0..mid {
                    first[cursor] = input_data[i].to_sample::<f32>();
                    cursor += 1;
                }
                cursor = 0;
                for i in mid..should_push_data_cnt {
                    second[cursor] = input_data[i].to_sample::<f32>();
                    cursor += 1;
                }
                chunk.commit_all();
            }
            Err(err) => {
                println!("[FileSrc] Failed to push sample: {}", err);
                return;
            }
        };

        loop {
            let one_round_needed_sample_cnt = Resampler::input_frames_max(&self.resampler) * Resampler::nbr_channels(&self.resampler);

            // check enough to resample
            if self.inner_consumer.slots() < one_round_needed_sample_cnt {
                // println!("[HAL] not enough samples to resample");
                return;
            }

            // Deinterleave
            match self.inner_consumer.read_chunk(one_round_needed_sample_cnt) {
                Ok(chunk) => {
                    let (first, second) = chunk.as_slices();
                    let mut cursor = 0;
                    for a in [first, second].iter() {
                        for v in a.iter() {
                            let chan = cursor % self.src_channels_cnt;
                            let idx = cursor / self.src_channels_cnt;
                            self.input_channels[chan][idx] = *v;
                            cursor += 1;
                        }
                    }
                    chunk.commit_all();
                }
                Err(err) => {
                    println!("[HAL] Error reading data {:?}", err);
                    return;
                }
            }

            // resample
            let ret = Resampler::process_into_buffer(&mut self.resampler, &mut self.input_channels, &mut self.output_channels, None);
            match ret {
                Ok((_read, written)) => {
                    self.handle_output(written);
                }
                Err(error) => {
                    println!("[HAL] Error processing data: {}", error);
                }
            }
        }
    }

    pub fn check_producer_full(&mut self) -> bool {
        let size = self.producer.slots();
        let needed = self.target_channels_cnt;
        needed >= size
    }

    fn handle_output(&mut self, written: usize) {
        // multi-chan algo: only dup first channel
        let mut not_sent = written;
        // println!("[HAL] not_sent {} samples to producer", not_sent);
        let first_channel = &self.output_channels[0];
        for idx in 0..first_channel.len() {
            if self.producer.slots() < self.target_channels_cnt {
                println!("[HAL] resample Output buffer full");
                break;
            }
            if not_sent == 0 {
                break;
            }
            for _ in 0..self.target_channels_cnt {
                if self.producer.push(first_channel[idx]).is_err() {
                    println!("[HAL] Error sending resample data to producer {}", idx);
                    break;
                }
            }
            not_sent -= 1;
        }
    }
}