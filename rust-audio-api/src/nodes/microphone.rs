use crate::nodes::resampler::{ResamplerState, RingIter};
use crate::types::{AUDIO_UNIT_SIZE, AudioUnit};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Stream, StreamConfig};
use dasp::signal::Signal;
use ringbuf::HeapRb;
use ringbuf::traits::{Observer, Producer, Split};

/// An audio source that captures sound from the system's default microphone.
///
/// It automatically handles sample rate conversion if the microphone's native
/// sample rate differs from the target sample rate.
pub struct MicrophoneNode {
    resampler: ResamplerState,
    _stream: Stream,
    gain: f32,
}

impl MicrophoneNode {
    /// Creates a new `MicrophoneNode` targeting the specified sample rate.
    ///
    /// # Parameters
    /// - `target_sample_rate`: The sample rate requested by the `AudioContext`.
    pub fn new(target_sample_rate: u32) -> Result<Self, anyhow::Error> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .expect("Microphone device not found");
        let supported_config = device.default_input_config()?;
        let input_rate = supported_config.sample_rate();
        let config: StreamConfig = supported_config.into();
        let channels = config.channels as usize;

        println!("Microphone sample rate: {:?}", input_rate);

        // Approx 1 second of buffer
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
            |err| eprintln!("Microphone capture error: {}", err),
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

    /// Sets the input gain for the microphone capture.
    pub fn set_gain(&mut self, gain: f32) {
        self.gain = gain;
    }

    #[inline(always)]
    pub fn process(&mut self, _input: Option<&AudioUnit>, output: &mut AudioUnit) {
        match &mut self.resampler {
            ResamplerState::Passthrough(iter) => {
                for out in output.iter_mut().take(AUDIO_UNIT_SIZE) {
                    *out = iter.next();
                }
            }
            ResamplerState::Resampling(converter) => {
                for out in output.iter_mut().take(AUDIO_UNIT_SIZE) {
                    *out = converter.next();
                }
            }
        }

        // Apply gain safely using dasp slice operations
        dasp::slice::map_in_place(&mut output[..], |frame| {
            [frame[0] * self.gain, frame[1] * self.gain]
        });
    }
}
