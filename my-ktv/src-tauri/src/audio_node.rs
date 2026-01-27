/***
 * @ Mod:       audio_node
 * @ Author:    Leon Lin
 * @ Date:      20260121
 */

pub mod fake_audio_wave_src;
pub mod file_src;
pub mod mic_src;
mod node_const;
pub mod speaker_dest;
mod utils;

use crate::audio_node::fake_audio_wave_src::FakeAudioWaveSRC;
use crate::audio_node::file_src::FileSrc;
use crate::audio_node::mic_src::MicSrc;
use crate::audio_node::speaker_dest::SpeakerDest;
use rtrb::Producer;

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
    FileSrc(FileSrc),
    SpeakerDest(SpeakerDest),
    MicSrc(MicSrc),
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
            AudioNodeEnum::FileSrc(node) => node.start(),
            AudioNodeEnum::SpeakerDest(node) => node.start(),
            AudioNodeEnum::MicSrc(node) => node.start(),
        }
    }

    fn stop(&mut self) {
        match self {
            AudioNodeEnum::FakeAudioWaveSRC(node) => node.stop(),
            AudioNodeEnum::FileSrc(node) => node.stop(),
            AudioNodeEnum::SpeakerDest(node) => node.stop(),
            AudioNodeEnum::MicSrc(node) => node.stop(),
        }
    }

    fn get_type(&self) -> AudioNodeType {
        match self {
            AudioNodeEnum::FakeAudioWaveSRC(node) => node.get_type(),
            AudioNodeEnum::FileSrc(node) => node.get_type(),
            AudioNodeEnum::SpeakerDest(node) => node.get_type(),
            AudioNodeEnum::MicSrc(node) => node.get_type(),
        }
    }

    fn get_state(&self) -> AudioNodeState {
        match self {
            AudioNodeEnum::FakeAudioWaveSRC(node) => node.get_state(),
            AudioNodeEnum::FileSrc(node) => node.get_state(),
            AudioNodeEnum::SpeakerDest(node) => node.get_state(),
            AudioNodeEnum::MicSrc(node) => node.get_state(),
        }
    }
}

pub fn connect(source: &mut AudioNodeEnum, dest: &mut AudioNodeEnum) -> Result<(), String> {
    match (source, dest) {
        (AudioNodeEnum::FakeAudioWaveSRC(src_inner), AudioNodeEnum::SpeakerDest(dest_inner)) => {
            transfer_producer(
                &mut src_inner.audio_producer,
                &mut dest_inner.audio_producer,
            )
        }

        (AudioNodeEnum::MicSrc(src_inner), AudioNodeEnum::SpeakerDest(dest_inner)) => {
            src_inner.input_producer_config = Option::from(dest_inner.config.clone());
            transfer_producer(
                &mut src_inner.audio_producer,
                &mut dest_inner.audio_producer,
            )
        }

        (AudioNodeEnum::FileSrc(src_inner), AudioNodeEnum::SpeakerDest(dest_inner)) => {
            transfer_producer(
                &mut src_inner.audio_producer,
                &mut dest_inner.audio_producer,
            )
        }

        _ => Err("no supported connection".to_string()),
    }
}

fn transfer_producer(
    src_producer_slot: &mut Option<Producer<f32>>,
    dest_producer_slot: &mut Option<Producer<f32>>,
) -> Result<(), String> {
    match dest_producer_slot.take() {
        Some(producer) => {
            *src_producer_slot = Some(producer);
            Ok(())
        }
        None => Err("No producer available in destination (maybe already connected?)".to_string()),
    }
}
