use rust_audio_api::nodes::OscillatorNode;
use rust_audio_api::types::empty_audio_unit;
use std::thread;
use std::time::Duration;

#[test]
fn test_oscillator_node_initialization() {
    let _osc_node = OscillatorNode::new(48000.0, 440.0);
    // Verifying instantiation and background thread starts without panic
}

#[test]
fn test_oscillator_node_set_gain() {
    let mut osc_node = OscillatorNode::new(48000.0, 440.0);
    let mut output = empty_audio_unit();

    // Give the background thread a moment to produce samples
    thread::sleep(Duration::from_millis(10));

    osc_node.set_gain(0.5);
    osc_node.process(None, &mut output);

    // The output should be active and scaled, verify it's not silent and bounded by gain
    let mut has_non_zero = false;
    for frame in output.iter() {
        if frame[0] != 0.0 || frame[1] != 0.0 {
            has_non_zero = true;
            assert!(frame[0].abs() <= 0.5);
            assert!(frame[1].abs() <= 0.5);
        }
    }
    assert!(has_non_zero, "Oscillator produced no output");
}

#[test]
fn test_oscillator_node_process_generates_signal() {
    let mut osc_node = OscillatorNode::new(48000.0, 440.0);
    let mut output = empty_audio_unit();

    // Give the background thread a moment to produce samples
    thread::sleep(Duration::from_millis(50));

    osc_node.process(None, &mut output);

    // Output should contain sine wave data
    let mut has_non_zero = false;
    let mut has_positive = false;
    let mut has_negative = false;

    for frame in output.iter() {
        if frame[0] != 0.0 {
            has_non_zero = true;
        }
        if frame[0] > 0.0 {
            has_positive = true;
        }
        if frame[0] < 0.0 {
            has_negative = true;
        }
    }

    assert!(has_non_zero, "Oscillator produced no output (all zeros)");
    // A sine wave over 64 frames (approx 1.3ms at 48kHz) might not alternate sign,
    // but pulling multiple units should eventually reveal the curve.

    let mut found_oscillation = has_positive && has_negative;
    if !found_oscillation {
        for _ in 0..10 {
            osc_node.process(None, &mut output);
            for frame in output.iter() {
                if frame[0] > 0.0 {
                    has_positive = true;
                }
                if frame[0] < 0.0 {
                    has_negative = true;
                }
            }
            if has_positive && has_negative {
                found_oscillation = true;
                break;
            }
        }
    }
    assert!(
        found_oscillation,
        "Oscillator signal did not alternate signs (not a wave)"
    );
}
