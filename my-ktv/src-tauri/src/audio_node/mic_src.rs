use crate::audio_node::utils::{generate_input_resolve_config, IOStreamConfig};
use crate::audio_node::{AudioNode, AudioNodeState, AudioNodeType};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, Stream, StreamConfig, StreamError};
use rtrb::Producer;

pub struct MicSrc {
    pub state: AudioNodeState,
    pub audio_producer: Option<Producer<f32>>,
    pub input_producer_config: Option<IOStreamConfig>,
    input_stream: Option<Stream>,
    device: cpal::Device,
    config: IOStreamConfig,
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

        Self {
            state: AudioNodeState::INITIALIZED,
            audio_producer: None,
            input_producer_config: None,
            input_stream: None,
            device: input_device,
            config: input_config,
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

            let stream = match input_config.sample_format {
                SampleFormat::F32 => self
                    .device
                    .build_input_stream(
                        &self.config.stream_config,
                        data_input_callback_creator::<f32>(
                            producer,
                            input_config.stream_config.clone(),
                            producer_config.stream_config,
                        ),
                        err_hdl_cb,
                        None,
                    )
                    .expect("failed to build input stream"),
                SampleFormat::I32 => self
                    .device
                    .build_input_stream(
                        &self.config.stream_config,
                        data_input_callback_creator::<i32>(
                            producer,
                            input_config.stream_config.clone(),
                            producer_config.stream_config,
                        ),
                        err_hdl_cb,
                        None,
                    )
                    .expect("failed to build input stream"),
                SampleFormat::I16 => self
                    .device
                    .build_input_stream(
                        &self.config.stream_config,
                        data_input_callback_creator::<i16>(
                            producer,
                            input_config.stream_config.clone(),
                            producer_config.stream_config,
                        ),
                        err_hdl_cb,
                        None,
                    )
                    .expect("failed to build input stream"),
                SampleFormat::U8 => self
                    .device
                    .build_input_stream(
                        &self.config.stream_config,
                        data_input_callback_creator::<u8>(
                            producer,
                            input_config.stream_config.clone(),
                            producer_config.stream_config,
                        ),
                        err_hdl_cb,
                        None,
                    )
                    .expect("failed to build input stream"),

                _ => panic!("MicSrc: cannot start, no producer sample format"),
            };

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

fn data_input_callback_creator<T>(
    mut producer: Producer<f32>,
    src_cfg: StreamConfig,
    target_cfg: StreamConfig,
) -> impl FnMut(&[T], &cpal::InputCallbackInfo)
where
    T: Sample,
    f32: FromSample<T>,
{
    let src_channels = src_cfg.channels as usize;
    let target_channels = target_cfg.channels as usize;

    let resample_ratio = src_cfg.sample_rate as f32 / target_cfg.sample_rate as f32;

    let mut current_input_idx_f32: f32 = 0.0;

    move |data: &[T], _: &cpal::InputCallbackInfo| {
        if producer.is_full() {
            // println!("[HAL] Buffer full");
            return;
        }

        let total_samples = data.len();
        let total_frames = total_samples / src_channels;

        // loop each frame => 2 item for 2 channel case
        while (current_input_idx_f32 as usize) < total_frames {
            // Decompose the current idx into integers and decimals.
            let first_idx = current_input_idx_f32 as usize;
            let fract = current_input_idx_f32 - first_idx as f32;
            // get next index
            let next_index = if first_idx + 1 < total_frames {
                // next item
                first_idx + 1
            } else {
                // edge case, don't move
                first_idx
            };

            // get value between first_idx and  next_index
            let frame_first_start = first_idx * src_channels;
            let frame_second_start = next_index * src_channels;

            let mut frame_first_sum = 0.0;
            let mut frame_second_sum = 0.0;

            for idx in 0..src_channels {
                frame_first_sum += data[frame_first_start + idx].to_sample::<f32>();
                frame_second_sum += data[frame_second_start + idx].to_sample::<f32>();
            }

            let frame_first_avg = frame_first_sum / src_channels as f32;
            let frame_second_avg = frame_second_sum / src_channels as f32;

            // Linear Interpolation
            let final_value = frame_first_avg + (frame_second_avg - frame_first_avg) * fract;

            // push frame into channel
            for _ in 0..target_channels {
                let _ = producer.push(final_value);
            }

            current_input_idx_f32 += resample_ratio;
        }

        current_input_idx_f32 -= total_frames as f32;
    }
}
