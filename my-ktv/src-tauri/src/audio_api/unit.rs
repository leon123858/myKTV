use crate::audio_api::audio_const::SAMPLES_PER_UNIT;
use dasp::interpolate::linear::Linear;
use dasp::Frame;
use dasp::Sample;
use dasp::{signal, slice, Signal};
use std::iter::Cloned;
use std::slice::Iter;

#[derive(Default)]
struct AudioUnitMeta {
    pub sample_rate: u32,
    pub channel_cnt: u32,
}

struct AudioUnit<S, const N: usize> {
    pub meta: AudioUnitMeta,
    pub frames: [[S; N]; SAMPLES_PER_UNIT],
}

impl<S, const N: usize> AudioUnit<S, N>
where
    S: Sample + std::ops::MulAssign,
    [S; N]: Frame<Sample = S> + Copy,
    [[S; N]; SAMPLES_PER_UNIT]: Signal<Frame = [S; N]>,
{
    pub fn new() -> Self {
        Self {
            meta: Default::default(),
            frames: [[S::EQUILIBRIUM; N]; SAMPLES_PER_UNIT],
        }
    }

    pub fn get_channel_len(&self) -> usize {
        N
    }

    pub fn fill_with_zero(&mut self) {
        slice::equilibrium(&mut self.frames);
    }

    pub fn gain(&mut self, scale: S) {
        slice::map_in_place(&mut self.frames, |frame| {
            frame.map(|mut sample| {
                sample *= scale;
                sample
            })
        });
    }

    pub fn raw_data(&mut self) -> &mut [[S; N]; SAMPLES_PER_UNIT] {
        &mut self.frames
    }

    pub fn get_signal(&self) -> signal::FromIterator<Cloned<Iter<'_, [S; N]>>> {
        signal::from_iter(self.frames.iter().cloned())
    }

    pub fn fill_from_signal<Sig>(&mut self, mut sig: Sig)
    where
        Sig: Signal<Frame = [S; N]>,
    {
        for frame in self.frames.iter_mut() {
            *frame = sig.next()
        }
    }
}
