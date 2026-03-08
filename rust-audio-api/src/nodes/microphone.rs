use crate::types::{AUDIO_UNIT_SIZE, AudioUnit};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Stream, StreamConfig};
use dasp::signal::{self, Signal};
use ringbuf::storage::Heap;
use ringbuf::traits::{Consumer, Observer, Producer, Split};
use ringbuf::wrap::caching::Caching;
use ringbuf::{HeapRb, SharedRb};
use std::sync::Arc;

pub struct MicrophoneNode {
    // 改為儲存原始 interleaved f32
    consumer: Caching<Arc<SharedRb<Heap<f32>>>, false, true>,
    _stream: Stream,
    gain: f32,
    input_channels: usize,
    input_sample_rate: u32,
    target_sample_rate: u32,
}

impl MicrophoneNode {
    pub fn new(target_sample_rate: u32) -> Result<Self, anyhow::Error> {
        let host = cpal::default_host();
        let device = host.default_input_device().expect("找不到麥克風設備");
        let supported_config = device.default_input_config()?;
        let input_rate = supported_config.sample_rate();
        let config: StreamConfig = supported_config.into();
        let channels = config.channels as usize;

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

        Ok(Self {
            consumer,
            _stream: stream,
            gain: 1.0,
            input_channels: channels,
            input_sample_rate: input_rate,
            target_sample_rate,
        })
    }

    pub fn set_gain(&mut self, gain: f32) {
        self.gain = gain;
    }

    #[inline(always)]
    pub fn process(&mut self, _input: Option<&AudioUnit>, output: &mut AudioUnit) {
        let fetch_size = AUDIO_UNIT_SIZE * self.input_channels;
        let mut raw_samples = vec![0.0; fetch_size];

        // 1. 取出 AUDIO_UNIT_SIZE * input_channels 數量的 frame
        for i in 0..fetch_size {
            raw_samples[i] = self.consumer.try_pop().unwrap_or(0.0);
        }

        // 2. 組合成原始 AudioUnit (長度 64)
        let mut source_unit = [[0.0; 2]; AUDIO_UNIT_SIZE];
        for i in 0..AUDIO_UNIT_SIZE {
            let offset = i * self.input_channels;
            if self.input_channels == 1 {
                source_unit[i][0] = raw_samples[offset];
                source_unit[i][1] = raw_samples[offset];
            } else if self.input_channels == 2 {
                source_unit[i][0] = raw_samples[offset];
                source_unit[i][1] = raw_samples[offset + 1];
            } else {
                let mut sum = 0.0;
                for c in 0..self.input_channels {
                    sum += raw_samples[offset + c];
                }
                let avg = sum / self.input_channels as f32;
                source_unit[i][0] = avg;
                source_unit[i][1] = avg;
            }
        }

        // 3. 透過 dasp 處理 resample
        if self.input_sample_rate != self.target_sample_rate {
            let iter = source_unit.into_iter().chain(std::iter::repeat([0.0; 2]));
            let sig = signal::from_iter(iter);

            let ring_buffer = dasp::ring_buffer::Fixed::from([[0.0; 2]; 100]);
            let sinc = dasp::interpolate::sinc::Sinc::new(ring_buffer);

            let mut resampler = sig.from_hz_to_hz(
                sinc,
                self.input_sample_rate as f64,
                self.target_sample_rate as f64,
            );

            for i in 0..AUDIO_UNIT_SIZE {
                let mut frame = resampler.next();
                frame[0] *= self.gain;
                frame[1] *= self.gain;
                output[i] = frame;
            }
        } else {
            for i in 0..AUDIO_UNIT_SIZE {
                output[i][0] = source_unit[i][0] * self.gain;
                output[i][1] = source_unit[i][1] * self.gain;
            }
        }
    }
}
