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

/// A node that reads and decodes audio from a file.
///
/// It uses the `symphonia` library to support various audio formats.
/// Like [`MicrophoneNode`][crate::nodes::MicrophoneNode], it handles
/// sample rate conversion automatically.
///
/// # Example
/// ```no_run
/// use rust_audio_api::nodes::{FileNode, NodeType};
/// use rust_audio_api::AudioContext;
///
/// let mut ctx = AudioContext::new().unwrap();
/// let sample_rate = ctx.sample_rate();
///
/// let dest_id = ctx.build_graph(|builder| {
///     let file = FileNode::new("music.mp3", sample_rate).unwrap();
///     builder.add_node(NodeType::File(file))
/// });
/// ```
pub struct FileNode {
    resampler: ResamplerState,
    gain: f32,
    _running: Arc<AtomicBool>,
}

impl FileNode {
    /// Creates a new `FileNode` for the specified file path.
    ///
    /// # Parameters
    /// - `file_path`: Path to the audio file.
    /// - `target_sample_rate`: Processing sample rate.
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
            .ok_or_else(|| anyhow::anyhow!("No audio track found"))?;

        let track_id = track.id;
        let mut decoder =
            symphonia::default::get_codecs().make(&track.codec_params, &decoder_opts)?;

        let channels = track.codec_params.channels.unwrap_or_default().count();
        let sample_rate = track.codec_params.sample_rate.unwrap_or(target_sample_rate);

        println!("Audio file sample rate: {:?}", sample_rate);

        // Create 2-second raw f32 buffer
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
            let ring_buffer = dasp::ring_buffer::Fixed::from([[0.0; 2]; AUDIO_UNIT_SIZE]);
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

    /// Sets the output gain for the file playback.
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

impl Drop for FileNode {
    fn drop(&mut self) {
        self._running.store(false, Ordering::Relaxed);
    }
}
