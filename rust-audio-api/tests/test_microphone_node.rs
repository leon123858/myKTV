use rust_audio_api::nodes::microphone::MicrophoneNode;
use rust_audio_api::types::AUDIO_UNIT_SIZE;

#[test]
fn test_microphone_node_creation() {
    // Like AudioContext, creating a microphone node might fail if no recording device is present.
    // We handle the result gracefully for CI/testing.
    let node_result = MicrophoneNode::new(44100);

    if let Ok(mut node) = node_result {
        node.set_gain(0.5);

        let mut output = [[0.0; 2]; AUDIO_UNIT_SIZE];
        // Ensure process does not panic
        node.process(None, &mut output);
    }
}
