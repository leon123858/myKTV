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

pub struct OscillatorNode {
    consumer: Caching<Arc<SharedRb<Heap<[f32; 2]>>>, false, true>,
    gain: f32,
    _running: Arc<AtomicBool>,
}

impl OscillatorNode {
    pub fn new(sample_rate: f64, frequency: f64) -> Self {
        // 設定 ringbuf，大概 0.5 秒的緩衝 (例如 48000 Hz => 24000)
        let capacity = (sample_rate * 0.5) as usize;
        let ringbuf = HeapRb::<[f32; 2]>::new(capacity);
        let (mut producer, consumer) = ringbuf.split();

        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();

        thread::spawn(move || {
            let mut sig = signal::rate(sample_rate).const_hz(frequency).sine();
            while running_clone.load(Ordering::Relaxed) {
                // 如果 buffer 滿了，稍微暫停一下讓 Audio Thread 消耗 (不 push，避免高 CPU 佔用)
                if producer.is_full() {
                    thread::sleep(Duration::from_millis(5));
                    continue;
                }

                let sample = sig.next() as f32;
                let frame = [sample, sample];
                let _ = producer.try_push(frame); // 如果滿了會直接忽略 (由上方的 sleep 處理主要回壓)
            }
        });

        Self {
            consumer,
            gain: 1.0,
            _running: running,
        }
    }

    pub fn set_gain(&mut self, gain: f32) {
        self.gain = gain;
    }

    /// Oscillator 是主動節點 (Source)，它不受 input 影響。
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
