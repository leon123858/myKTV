use crate::audio_node::node_const::RING_BUFFER_CAPACITY;
use crate::audio_node::utils::{generate_output_resolve_config, IOStreamConfig};
use crate::audio_node::{AudioNode, AudioNodeState, AudioNodeType};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, Stream, StreamError};
use rtrb::{Consumer, Producer, RingBuffer};

pub struct SpeakerDest {
    pub state: AudioNodeState,
    pub audio_producer: Option<Producer<f32>>,
    pub output_stream: Stream,
    pub config: IOStreamConfig,
}

impl AudioNode for SpeakerDest {
    fn init() -> Self {
        let host = cpal::default_host();
        println!("[HAL] Audio Host: {:?}", host.id());

        // 獲取輸出設備 (DAC)
        let output_device = host
            .default_output_device()
            .expect("no output device available");
        println!("[HAL] Output Device: {:?}", output_device.description().unwrap().name());

        // negotiation function
        let mut resolve_config_fn = generate_output_resolve_config("Speaker".parse().unwrap());

        // 協商並建立輸出流
        let output_config =
            resolve_config_fn(&output_device).expect("failed to resolve output device config");
        println!("[HAL] Negotiated Output Config: {:?}", output_config);

        // 建立 Lock-free Ring Buffer
        let (producer, consumer) = RingBuffer::<f32>::new(RING_BUFFER_CAPACITY);

        println!("[HAL] New Producer Size: {:?}", producer.slots());

        let output_stream_ret = match output_config.sample_format {
            cpal::SampleFormat::F32 => output_device.build_output_stream(
                &output_config.stream_config,
                data_hdl_cb_creator::<f32>(consumer),
                err_hdl_cb,
                None, // Timeout: blocking negotiation
            ),
            cpal::SampleFormat::I32 => output_device.build_output_stream(
                &output_config.stream_config,
                data_hdl_cb_creator::<i32>(consumer),
                err_hdl_cb,
                None, // Timeout: blocking negotiation
            ),
            cpal::SampleFormat::I16 => output_device.build_output_stream(
                &output_config.stream_config,
                data_hdl_cb_creator::<i16>(consumer),
                err_hdl_cb,
                None, // Timeout: blocking negotiation
            ),
            cpal::SampleFormat::U8 => output_device.build_output_stream(
                &output_config.stream_config,
                data_hdl_cb_creator::<u8>(consumer),
                err_hdl_cb,
                None, // Timeout: blocking negotiation
            ),
            _ => panic!("Unsupported format"),
        };

        let output_stream = output_stream_ret.expect("output stream created error");

        Self {
            state: AudioNodeState::INITIALIZED,
            audio_producer: Option::from(producer),
            output_stream,
            config: output_config,
        }
    }

    fn start(&mut self) {
        self.output_stream.play().expect("failed to start stream");
        self.state = AudioNodeState::RUNNING
    }

    fn stop(&mut self) {
        self.output_stream.pause().expect("failed to start stream");
        self.state = AudioNodeState::STOPPED
    }

    fn get_type(&self) -> AudioNodeType {
        AudioNodeType::DESTINATION
    }

    fn get_state(&self) -> AudioNodeState {
        self.state.clone()
    }
}

fn err_hdl_cb(err: StreamError) {
    eprintln!("[HAL] Output Stream Error: {}", err);
}

pub fn data_hdl_cb_creator<T>(
    mut consumer: Consumer<f32>,
) -> impl FnMut(&mut [T], &cpal::OutputCallbackInfo)
where
    T: Sample + FromSample<f32>,
{
    move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
        let n = data.len();

        match consumer.read_chunk(n) {
            Ok(chunk) => {
                let (first, second) = chunk.as_slices();
                let first_len = first.len();

                for (dest, &src) in data[..first_len].iter_mut().zip(first.iter()) {
                    *dest = T::from_sample(src);
                }

                if !second.is_empty() {
                    for (dest, &src) in data[first_len..].iter_mut().zip(second.iter()) {
                        *dest = T::from_sample(src);
                    }
                }

                chunk.commit_all();
            }
            Err(_) => {
                data.fill(T::EQUILIBRIUM);
            }
        }
    }
}
