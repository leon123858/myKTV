use crossbeam_channel::unbounded;
use rust_audio_api::graph::{ControlMessage, GraphBuilder, NodeParameter};
use rust_audio_api::nodes::{GainNode, MixerNode, NodeType, OscillatorNode};
use rust_audio_api::types::{AUDIO_UNIT_SIZE, empty_audio_unit};

#[test]
fn test_graph_builder_and_static_graph() {
    let mut builder = GraphBuilder::new();

    // 1. Add nodes
    let osc_node = NodeType::Oscillator(OscillatorNode::new(48000.0, 440.0));
    let osc_id = builder.add_node(osc_node);

    let gain_node = NodeType::Gain(GainNode::new(0.5));
    let gain_id = builder.add_node(gain_node);

    // 2. Connect (Oscillator -> Gain)
    builder.connect(osc_id, gain_id);

    // 3. Build StaticGraph
    let (_tx, rx) = unbounded();
    let mut graph = builder.build(gain_id, rx);

    // 4. Pull next unit
    let output = graph.pull_next_unit();

    // Simply verify we get valid output of size AUDIO_UNIT_SIZE
    assert_eq!(output.len(), AUDIO_UNIT_SIZE);
}

#[test]
fn test_graph_control_message_routing() {
    let mut builder = GraphBuilder::new();

    // Add a MixerNode
    let mixer = NodeType::Mixer(MixerNode::new());
    let mixer_id = builder.add_node(mixer);

    let (tx, rx) = unbounded();
    let mut graph = builder.build(mixer_id, rx);

    // Send a message to change volume
    tx.send(ControlMessage::SetParameter(
        mixer_id,
        NodeParameter::Gain(0.2),
    ))
    .unwrap();

    // Pull one unit to process the message and compute output
    // Initially mixer input is None, so output should be silence
    let output = graph.pull_next_unit();
    let expected = empty_audio_unit();
    assert_eq!(output, &expected);

    // But internally the Mixer Node's gain parameter should have been updated to 0.2.
    // Testing internal state is tricky via public interface without outputs, but
    // the code shouldn't panic and gracefully handled the message.
}
