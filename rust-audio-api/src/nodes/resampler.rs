use dasp::signal::Signal;
use ringbuf::SharedRb;
use ringbuf::storage::Heap;
use ringbuf::traits::Consumer;
use ringbuf::wrap::caching::Caching;
use std::sync::Arc;
pub struct RingIter {
    pub consumer: Caching<Arc<SharedRb<Heap<f32>>>, false, true>,
    pub channels: usize,
}

impl Signal for RingIter {
    type Frame = [f32; 2];

    #[inline(always)]
    fn next(&mut self) -> Self::Frame {
        let mut raw = [0.0; 8];
        for i in raw.iter_mut().take(self.channels) {
            *i = self.consumer.try_pop().unwrap_or(0.0);
        }

        if self.channels == 1 {
            [raw[0], raw[0]]
        } else if self.channels == 2 {
            [raw[0], raw[1]]
        } else {
            let mut sum = 0.0;
            for i in raw.iter().take(self.channels) {
                sum += i;
            }
            let avg = sum / self.channels as f32;
            [avg, avg]
        }
    }
}

pub enum ResamplerState {
    Passthrough(RingIter),
    Resampling(Box<dyn Signal<Frame = [f32; 2]> + Send>),
}
