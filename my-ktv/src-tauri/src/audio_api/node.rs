mod simulated_audio_wave_src;

use crate::audio_api::node::simulated_audio_wave_src::SimulatedAudioWaveSRC;

#[derive(Clone, Copy, Debug)]
pub enum AudioNodeState {
    INITIALIZED,
    RUNNING,
    STOPPED,
}

#[derive(Clone, Copy, Debug)]
pub enum AudioNodeModel {
    PULL,
    PUSH,
}

#[derive(Clone, Copy, Debug)]
pub enum AudioNodeName {
    SimulatedAudioWaveSRC
}

pub enum AudioNodeInterface {
    SimulatedAudioWaveSRC(SimulatedAudioWaveSRC)
}

pub trait AudioNode {
    fn init() -> Self;
    fn execute(&mut self);
    fn stop(&mut self);
    fn get_model(&self) -> AudioNodeModel;
    fn get_type(&self) -> AudioNodeName;
    fn get_state(&self) -> AudioNodeState;
    fn connect(&mut self, node: AudioNodeInterface, idx: u8);
    fn disconnect(&mut self, node: AudioNodeInterface, idx: u8);
}

impl AudioNode for AudioNodeInterface {
    fn init() -> Self {
        todo!()
    }

    fn execute(&mut self) {
        todo!()
    }

    fn stop(&mut self) {
        todo!()
    }

    fn get_model(&self) -> AudioNodeModel {
        todo!()
    }

    fn get_type(&self) -> AudioNodeName {
        todo!()
    }

    fn get_state(&self) -> AudioNodeState {
        todo!()
    }

    fn connect(&mut self, node: AudioNodeInterface, idx: u8) {
        todo!()
    }

    fn disconnect(&mut self, node: AudioNodeInterface, idx: u8) {
        todo!()
    }
}
