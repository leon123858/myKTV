use rust_audio_api::nodes::GainNode;
use rust_audio_api::types::{AUDIO_UNIT_SIZE, empty_audio_unit};

#[test]
fn test_gain_node_initialization() {
    let _gain_node = GainNode::new(0.5);
    // Just verifying struct can be instantiated without panic
}

#[test]
fn test_gain_node_set_gain() {
    let mut gain_node = GainNode::new(0.5);
    let mut output = empty_audio_unit();
    let mut input = empty_audio_unit();
    for i in 0..AUDIO_UNIT_SIZE {
        input[i] = [1.0, 1.0];
    }

    // First verify initial gain
    gain_node.process(Some(&input), &mut output);
    assert_eq!(output[0], [0.5, 0.5]);

    // Update gain and verify
    gain_node.set_gain(0.2);
    gain_node.process(Some(&input), &mut output);
    assert_eq!(output[0], [0.2, 0.2]);
}

#[test]
fn test_gain_node_process_scale_up() {
    let mut gain_node = GainNode::new(2.0);
    let mut output = empty_audio_unit();
    let mut input = empty_audio_unit();
    for i in 0..AUDIO_UNIT_SIZE {
        input[i] = [0.5, 0.5];
    }

    gain_node.process(Some(&input), &mut output);

    for i in 0..AUDIO_UNIT_SIZE {
        assert_eq!(output[i], [1.0, 1.0]);
    }
}

#[test]
fn test_gain_node_process_scale_down() {
    let mut gain_node = GainNode::new(0.25);
    let mut output = empty_audio_unit();
    let mut input = empty_audio_unit();
    for i in 0..AUDIO_UNIT_SIZE {
        input[i] = [0.8, 0.8];
    }

    gain_node.process(Some(&input), &mut output);

    // Using a tolerance to avoid precision errors in float comparisons
    for i in 0..AUDIO_UNIT_SIZE {
        assert!((output[i][0] - 0.2).abs() < f32::EPSILON);
        assert!((output[i][1] - 0.2).abs() < f32::EPSILON);
    }
}

#[test]
fn test_gain_node_process_phase_inversion() {
    let mut gain_node = GainNode::new(-1.0);
    let mut output = empty_audio_unit();
    let mut input = empty_audio_unit();
    for i in 0..AUDIO_UNIT_SIZE {
        input[i] = [0.5, -0.3];
    }

    gain_node.process(Some(&input), &mut output);

    for i in 0..AUDIO_UNIT_SIZE {
        assert_eq!(output[i], [-0.5, 0.3]);
    }
}

#[test]
fn test_gain_node_process_without_input_outputs_silence() {
    let mut gain_node = GainNode::new(0.5);
    let mut output = empty_audio_unit();

    // Make output non-zero initially to ensure it gets explicitly cleared to silence
    for i in 0..AUDIO_UNIT_SIZE {
        output[i] = [1.0, 1.0];
    }

    gain_node.process(None, &mut output);

    let expected = empty_audio_unit();
    assert_eq!(output, expected);
}
