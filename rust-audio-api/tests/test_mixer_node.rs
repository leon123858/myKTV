use rust_audio_api::nodes::MixerNode;
use rust_audio_api::types::{AUDIO_UNIT_SIZE, empty_audio_unit};

#[test]
fn test_mixer_node_initialization() {
    let mixer_node = MixerNode::new();
    let custom_mixer_node = MixerNode::with_gain(0.5);
    // Verifying instantiation and basic APIs
    let _ = mixer_node;
    let _ = custom_mixer_node;
}

#[test]
fn test_mixer_node_set_gain() {
    let mut mixer_node = MixerNode::new(); // Starts at 1.0
    let mut output = empty_audio_unit();
    let mut input = empty_audio_unit();
    input[0] = [0.5, 0.5];

    // Default passthrough behavior (1.0)
    mixer_node.process(Some(&input), &mut output);
    assert_eq!(output[0], [0.5, 0.5]);

    // Apply new gain
    mixer_node.set_gain(2.0);
    mixer_node.process(Some(&input), &mut output);
    assert_eq!(output[0], [1.0, 1.0]);
}

#[test]
fn test_mixer_node_process_passthrough() {
    let mut mixer_node = MixerNode::with_gain(1.0);
    let mut output = empty_audio_unit();
    let mut input = empty_audio_unit();

    input[0] = [0.25, -0.25];
    input[1] = [0.8, -0.8];

    mixer_node.process(Some(&input), &mut output);

    assert_eq!(output[0], [0.25, -0.25]);
    assert_eq!(output[1], [0.8, -0.8]);
}

#[test]
fn test_mixer_node_process_clipping() {
    let mut mixer_node = MixerNode::with_gain(1.0);
    let mut output = empty_audio_unit();
    let mut input = empty_audio_unit();

    // Hard limits check beyond 1.0 and -1.0
    input[0] = [1.5, 2.0];
    input[1] = [-1.5, -5.0];

    mixer_node.process(Some(&input), &mut output);

    assert_eq!(output[0], [1.0, 1.0]);
    assert_eq!(output[1], [-1.0, -1.0]);
}

#[test]
fn test_mixer_node_process_gain_with_clipping() {
    let mut mixer_node = MixerNode::with_gain(2.0);
    let mut output = empty_audio_unit();
    let mut input = empty_audio_unit();

    // Normal scaling under bounds
    input[0] = [0.25, -0.25]; // -> [0.5, -0.5]
    // Scaling over bounds
    input[1] = [0.8, -0.8]; // -> [1.6, -1.6] -> clipped [1.0, -1.0]

    mixer_node.process(Some(&input), &mut output);

    assert_eq!(output[0], [0.5, -0.5]);
    assert_eq!(output[1], [1.0, -1.0]);
}

#[test]
fn test_mixer_node_process_without_input_outputs_silence() {
    let mut mixer_node = MixerNode::new();
    let mut output = empty_audio_unit();

    for i in 0..AUDIO_UNIT_SIZE {
        output[i] = [1.0, 1.0];
    }

    mixer_node.process(None, &mut output);

    let expected = empty_audio_unit();
    assert_eq!(output, expected);
}
