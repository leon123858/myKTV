use crate::nodes::resampler::{ResamplerState, RingIter};
use crate::types::{AUDIO_UNIT_SIZE, AudioUnit};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Stream, StreamConfig};
use dasp::signal::Signal;
use ringbuf::HeapRb;
use ringbuf::traits::{Observer, Producer, Split};

pub struct MicrophoneNode {
    resampler: ResamplerState,
    _stream: Stream,
    gain: f32,
}

impl MicrophoneNode {
    pub fn new(target_sample_rate: u32) -> Result<Self, anyhow::Error> {
        let host = cpal::default_host();
        let device = host.default_input_device().expect("找不到麥克風設備");
        let supported_config = device.default_input_config()?;
        let input_rate = supported_config.sample_rate();
        let config: StreamConfig = supported_config.into();
        let channels = config.channels as usize;

        println!("麥克風採樣率: {:?}", input_rate);

        // 約 1 秒的緩衝量
        let capacity = input_rate as usize * channels;
        let ringbuf = HeapRb::<f32>::new(capacity);
        let (mut producer, consumer) = ringbuf.split();

        let stream = device.build_input_stream(
            &config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                for &sample in data {
                    if !producer.is_full() {
                        let _ = producer.try_push(sample);
                    }
                }
            },
            |err| eprintln!("麥克風擷取發生錯誤: {}", err),
            None,
        )?;

        stream.play()?;

        let ring_iter = RingIter { consumer, channels };

        let resampler = if input_rate != target_sample_rate {
            let ring_buffer = dasp::ring_buffer::Fixed::from([[0.0; 2]; 100]);
            let sinc = dasp::interpolate::sinc::Sinc::new(ring_buffer);
            let converter =
                ring_iter.from_hz_to_hz(sinc, input_rate as f64, target_sample_rate as f64);
            ResamplerState::Resampling(Box::new(converter))
        } else {
            ResamplerState::Passthrough(ring_iter)
        };

        Ok(Self {
            resampler,
            _stream: stream,
            gain: 1.0,
        })
    }

    pub fn set_gain(&mut self, gain: f32) {
        self.gain = gain;
    }

    #[inline(always)]
    pub fn process(&mut self, _input: Option<&AudioUnit>, output: &mut AudioUnit) {
        match &mut self.resampler {
            ResamplerState::Passthrough(iter) => {
                for i in 0..AUDIO_UNIT_SIZE {
                    let mut frame = iter.next();
                    frame[0] *= self.gain;
                    frame[1] *= self.gain;
                    output[i] = frame;
                }
            }
            ResamplerState::Resampling(converter) => {
                for i in 0..AUDIO_UNIT_SIZE {
                    let mut frame = converter.next();
                    frame[0] *= self.gain;
                    frame[1] *= self.gain;
                    output[i] = frame;
                }
            }
        }
    }
}
