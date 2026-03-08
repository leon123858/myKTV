use crate::nodes::resampler::{ResamplerState, RingIter};
use crate::types::{AUDIO_UNIT_SIZE, AudioUnit};
use dasp::signal::Signal;
use ringbuf::HeapRb;
use ringbuf::traits::{Observer, Producer, Split};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

pub struct FileNode {
    resampler: ResamplerState,
    gain: f32,
    _running: Arc<AtomicBool>,
}

impl FileNode {
    pub fn new(file_path: &str, target_sample_rate: u32) -> Result<Self, anyhow::Error> {
        let file = std::fs::File::open(file_path)?;
        let mss = MediaSourceStream::new(Box::new(file), Default::default());

        let hint = Hint::new();
        let format_opts = FormatOptions::default();
        let metadata_opts = MetadataOptions::default();
        let decoder_opts = DecoderOptions::default();

        let probed =
            symphonia::default::get_probe().format(&hint, mss, &format_opts, &metadata_opts)?;

        let mut format = probed.format;

        let track = format
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("未找到音訊軌"))?;

        let track_id = track.id;
        let mut decoder =
            symphonia::default::get_codecs().make(&track.codec_params, &decoder_opts)?;

        let channels = track.codec_params.channels.unwrap_or_default().count();
        let sample_rate = track.codec_params.sample_rate.unwrap_or(target_sample_rate);

        println!("音檔採樣率: {:?}", sample_rate);

        // 建立 2 秒的 raw f32 緩衝
        let capacity = sample_rate as usize * channels * 2;
        let ringbuf = HeapRb::<f32>::new(capacity);
        let (mut producer, consumer) = ringbuf.split();

        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();

        thread::spawn(move || {
            let mut sample_buf = None;

            while running_clone.load(Ordering::Relaxed) {
                if producer.is_full() {
                    thread::sleep(Duration::from_millis(10));
                    continue;
                }

                let packet = match format.next_packet() {
                    Ok(p) => p,
                    Err(_) => break, // EOF or Error
                };

                if packet.track_id() != track_id {
                    continue;
                }

                let decoded = match decoder.decode(&packet) {
                    Ok(d) => d,
                    Err(_) => continue,
                };

                if sample_buf.is_none() {
                    let spec = *decoded.spec();
                    let duration = decoded.capacity() as u64;
                    sample_buf = Some(SampleBuffer::<f32>::new(duration, spec));
                }

                let buf = sample_buf.as_mut().unwrap();
                buf.copy_interleaved_ref(decoded);

                let samples = buf.samples();

                for &sample in samples {
                    if !running_clone.load(Ordering::Relaxed) {
                        break;
                    }
                    while producer.is_full() && running_clone.load(Ordering::Relaxed) {
                        thread::sleep(Duration::from_millis(1));
                    }
                    let _ = producer.try_push(sample);
                }
            }
        });

        let ring_iter = RingIter { consumer, channels };

        let resampler = if sample_rate != target_sample_rate {
            let ring_buffer = dasp::ring_buffer::Fixed::from([[0.0; 2]; 100]);
            let sinc = dasp::interpolate::sinc::Sinc::new(ring_buffer);
            let converter =
                ring_iter.from_hz_to_hz(sinc, sample_rate as f64, target_sample_rate as f64);
            ResamplerState::Resampling(Box::new(converter))
        } else {
            ResamplerState::Passthrough(ring_iter)
        };

        Ok(Self {
            resampler,
            gain: 1.0,
            _running: running,
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

impl Drop for FileNode {
    fn drop(&mut self) {
        self._running.store(false, Ordering::Relaxed);
    }
}
