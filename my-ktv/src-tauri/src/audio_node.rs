pub mod fake_audio_wave_src;
pub mod speaker_dest;

use crate::audio_node::fake_audio_wave_src::FakeAudioWaveSRC;
use crate::audio_node::speaker_dest::SpeakerDest;

pub const RING_BUFFER_CAPACITY: usize = 65536;

#[derive(Clone, Copy, Debug)]
pub enum AudioNodeType {
    SOURCE,
    GAIN,
    DESTINATION,
}
#[derive(Clone, Copy, Debug)]
pub enum AudioNodeState {
    INITIALIZED,
    RUNNING,
    STOPPED,
}

pub enum AudioNodeEnum {
    FakeAudioWaveSRC(FakeAudioWaveSRC),
    SpeakerDest(SpeakerDest),
}

pub trait AudioNode {
    fn init() -> Self;
    fn start(&mut self);
    fn stop(&mut self);
    fn get_type(&self) -> AudioNodeType;
    fn get_state(&self) -> AudioNodeState;
}

impl AudioNode for AudioNodeEnum {
    fn init() -> Self {
        panic!("please init by itself");
    }

    fn start(&mut self) {
        match self {
            AudioNodeEnum::FakeAudioWaveSRC(node) => node.start(),
            AudioNodeEnum::SpeakerDest(node) => node.start(),
        }
    }

    fn stop(&mut self) {
        match self {
            AudioNodeEnum::FakeAudioWaveSRC(node) => node.stop(),
            AudioNodeEnum::SpeakerDest(node) => node.stop(),
        }
    }

    fn get_type(&self) -> AudioNodeType {
        match self {
            AudioNodeEnum::FakeAudioWaveSRC(node) => node.get_type(),
            AudioNodeEnum::SpeakerDest(node) => node.get_type(),
        }
    }

    fn get_state(&self) -> AudioNodeState {
        match self {
            AudioNodeEnum::FakeAudioWaveSRC(node) => node.get_state(),
            AudioNodeEnum::SpeakerDest(node) => node.get_state(),
        }
    }
}

pub fn connect(source: &mut AudioNodeEnum, dest: &mut AudioNodeEnum) -> Result<(), String> {
    match (source, dest) {
        (AudioNodeEnum::FakeAudioWaveSRC(src_inner), AudioNodeEnum::SpeakerDest(dest_inner)) => {
            if let Some(producer) = dest_inner.audio_producer.take() {
                src_inner.audio_producer = Some(producer);
                Ok(())
            } else {
                Err("No producer available in dest".to_string())
            }
        }

        _ => Err("no supported connection".to_string()),
    }
}
