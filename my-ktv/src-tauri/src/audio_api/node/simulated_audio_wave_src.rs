use crate::audio_api::node::{AudioNode, AudioNodeInterface, AudioNodeModel, AudioNodeName, AudioNodeState};
use crate::audio_api::node::AudioNodeModel::PULL;
use crate::audio_node::AudioNodeType::SOURCE;

pub struct SimulatedAudioWaveSRC {
    state: AudioNodeState,
}

impl AudioNode for SimulatedAudioWaveSRC {
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
        PULL
    }

    fn get_type(&self) -> AudioNodeName {
        AudioNodeName::SimulatedAudioWaveSRC
    }

    fn get_state(&self) -> AudioNodeState {
        self.state
    }

    fn connect(&mut self, node: AudioNodeInterface, idx: u8) {
        todo!()
    }

    fn disconnect(&mut self, node: AudioNodeInterface, idx: u8) {
        todo!()
    }
} 