use crate::types::AudioUnit;
use dasp::signal::{self, Signal};
use ringbuf::storage::Heap;
use ringbuf::traits::{Consumer, Observer, Producer, Split};
use ringbuf::wrap::caching::Caching;
use ringbuf::{HeapRb, SharedRb};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

/// An audio source that generates periodic waveforms.
///
/// Currently, it generates a sine wave at the specified frequency.
/// It runs a background thread to generate samples and uses a ring buffer
/// to communicate with the audio processing thread.
pub struct OscillatorNode {
    consumer: Caching<Arc<SharedRb<Heap<[f32; 2]>>>, false, true>,
    gain: f32,
    _running: Arc<AtomicBool>,
}

impl OscillatorNode {
    /// Creates a new `OscillatorNode` with the given sample rate and frequency.
    ///
    /// # Parameters
    /// - `sample_rate`: The target sample rate (e.g., 44100.0).
    /// - `frequency`: The frequency of the sine wave in Hz (e.g., 440.0).
    pub fn new(sample_rate: f64, frequency: f64) -> Self {
        // Set ringbuf capacity for approx 0.5s buffer (e.g., 48000 Hz => 24000)
        let capacity = (sample_rate * 0.5) as usize;
        let ringbuf = HeapRb::<[f32; 2]>::new(capacity);
        let (mut producer, consumer) = ringbuf.split();

        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();

        thread::spawn(move || {
            let mut sig = signal::rate(sample_rate).const_hz(frequency).sine();
            while running_clone.load(Ordering::Relaxed) {
                // If buffer is full, sleep briefly to let Audio Thread consume (prevents high CPU usage)
                if producer.is_full() {
                    thread::sleep(Duration::from_millis(5));
                    continue;
                }

                let sample = sig.next() as f32;
                let frame = [sample, sample];
                let _ = producer.try_push(frame); // Ignore if full (sleep handles backpressure)
            }
        });

        Self {
            consumer,
            gain: 1.0,
            _running: running,
        }
    }

    /// Sets the output gain (volume) for this oscillator.
    pub fn set_gain(&mut self, gain: f32) {
        self.gain = gain;
    }

    /// Oscillator is an active node (Source); it is unaffected by input.
    #[inline(always)]
    pub fn process(&mut self, _input: Option<&AudioUnit>, output: &mut AudioUnit) {
        dasp::slice::map_in_place(&mut output[..], |_| {
            if let Some(sample) = self.consumer.try_pop() {
                [sample[0] * self.gain, sample[1] * self.gain]
            } else {
                [0.0, 0.0]
            }
        });
    }
}

impl Drop for OscillatorNode {
    fn drop(&mut self) {
        self._running.store(false, Ordering::Relaxed);
    }
}
