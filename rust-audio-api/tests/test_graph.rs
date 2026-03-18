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

#[test]
fn test_graph_feedback_loop() {
    let mut builder = GraphBuilder::new();

    // Setup: Gain1 (input) -> Gain2 (output)
    // Feedback: Gain2 -> Gain1
    let g1_id = builder.add_node(NodeType::Gain(GainNode::new(1.0)));
    let g2_id = builder.add_node(NodeType::Gain(GainNode::new(0.5)));

    builder.connect(g1_id, g2_id);
    builder.connect_feedback(g2_id, g1_id);

    let (_tx, rx) = unbounded();
    let mut graph = builder.build(g2_id, rx);

    // Initial state: Gain1 input is silence, so Gain1 out is silence, Gain2 out is silence.
    let output1 = graph.pull_next_unit();
    assert_eq!(output1, &empty_audio_unit());

    // Pull again. Gain1 should now have Gain2's previous output (silence) as input.
    // This is hard to verify value-wise without an oscillator input, but
    // the fact it doesn't panic on a cycle is the primary verification of connect_feedback.
    let _output2 = graph.pull_next_unit();
}

#[test]
fn test_graph_multiple_inputs_to_gain() {
    let mut builder = GraphBuilder::new();

    // Two oscillators into one Gain node
    let osc1 = builder.add_node(NodeType::Oscillator(OscillatorNode::new(48000.0, 440.0)));
    let osc2 = builder.add_node(NodeType::Oscillator(OscillatorNode::new(48000.0, 880.0)));
    let gain = builder.add_node(NodeType::Gain(GainNode::new(1.0)));

    builder.connect(osc1, gain);
    builder.connect(osc2, gain);

    let (_tx, rx) = unbounded();
    let mut graph = builder.build(gain, rx);

    // This should not panic
    let _output = graph.pull_next_unit();
}
