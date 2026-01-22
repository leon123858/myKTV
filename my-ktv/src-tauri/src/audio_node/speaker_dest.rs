use crate::audio_node::node_const::{
     RING_BUFFER_CAPACITY,
};
use crate::audio_node::{AudioNode, AudioNodeState, AudioNodeType};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, SampleRate, Stream, StreamConfig, StreamError};
use rtrb::{Consumer, Producer, RingBuffer};
use crate::audio_node::utils::{generate_output_resolve_config};

pub struct SpeakerDest {
    pub state: AudioNodeState,
    pub audio_producer: Option<Producer<f32>>,
    pub output_stream: Stream,
    pub config: StreamConfig,
}

impl AudioNode for SpeakerDest {
    fn init() -> Self {
        let host = cpal::default_host();
        println!("[HAL] Audio Host: {:?}", host.id());

        // 獲取輸出設備 (DAC)
        let output_device = host
            .default_output_device()
            .expect("no output device available");
        println!("[HAL] Output Device: {:?}", output_device.name());

        // negotiation function
        let mut resolve_config_fn = generate_output_resolve_config("Speaker".parse().unwrap());

        // 協商並建立輸出流
        let output_config =
            resolve_config_fn(&output_device).expect("failed to resolve output device config");
        println!("[HAL] Negotiated Output Config: {:?}", output_config);

        // 建立 Lock-free Ring Buffer
        let (producer, consumer) = RingBuffer::<f32>::new(RING_BUFFER_CAPACITY);

        println!("[HAL] New Producer Size: {:?}", producer.slots());

        let output_stream_ret = output_device.build_output_stream(
            &output_config,
            data_hdl_cb_creator(consumer),
            err_hdl_cb,
            None, // Timeout: blocking negotiation
        );

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

fn data_hdl_cb_creator(
    mut consumer: Consumer<f32>,
) -> impl FnMut(&mut [f32], &cpal::OutputCallbackInfo) {
    move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {

        // read ring buffer data and send to output stream data
        let read_chunk_result = consumer.read_chunk(data.len());
        match read_chunk_result {
            Ok(chunk) => {
                // read chunk may be two split slice in ring buffer
                // because ring buffer's ring back property
                let (first, second) = chunk.as_slices();

                let first_len = first.len();
                let second_len = second.len();
                let mut data_idx = 0;
                for i in 0..first_len {
                    data[data_idx] = first[i];
                    data_idx += 1
                }
                for i in 0..second_len {
                    data[data_idx] = second[i];
                    data_idx += 1
                }
                assert_eq!(data_idx, data.len());

                // move ring buf ptr
                chunk.commit_all();
            }
            Err(err) => {
                // println!("chunk read error: {:?}", err);
                data.fill(0.0);
                return;
            }
        }
    }
}