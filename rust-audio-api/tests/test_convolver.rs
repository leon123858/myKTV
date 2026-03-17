use rust_audio_api::nodes::ConvolverNode;
use rust_audio_api::types::{AUDIO_UNIT_SIZE, empty_audio_unit};

#[test]
fn test_convolver_impulse() {
    let ir = vec![[1.0, 1.0]];
    let mut convolver = ConvolverNode::new(&ir);

    let mut input = empty_audio_unit();
    input[0] = [0.5, -0.5];
    input[10] = [1.0, 0.0];

    let mut output = empty_audio_unit();
    convolver.process(Some(&input), &mut output);

    assert!((output[0][0] - 0.5).abs() < 1e-5);
    assert!((output[0][1] - (-0.5)).abs() < 1e-5);

    for i in 1..AUDIO_UNIT_SIZE {
        assert!((output[i][0] - input[i][0]).abs() < 1e-5);
        assert!((output[i][1] - input[i][1]).abs() < 1e-5);
    }
}

#[test]
fn test_convolver_delayed_impulse() {
    let mut ir = vec![[0.0, 0.0]; 6];
    ir[5] = [1.0, 1.0];

    let mut convolver = ConvolverNode::new(&ir);

    let mut input = empty_audio_unit();
    input[0] = [0.5, -0.5];
    input[10] = [1.0, 0.2];
    input[63] = [0.1, 0.1]; // Will spill over to block 2

    let mut output = empty_audio_unit();
    convolver.process(Some(&input), &mut output);

    assert!((output[5][0] - 0.5).abs() < 1e-5);
    assert!((output[5][1] - (-0.5)).abs() < 1e-5);

    assert!((output[15][0] - 1.0).abs() < 1e-5);
    assert!((output[15][1] - 0.2).abs() < 1e-5);

    for i in 0..AUDIO_UNIT_SIZE {
        if i != 5 && i != 15 {
            assert!((output[i][0]).abs() < 1e-5);
        }
    }

    // Process block 2 (empty input)
    let input2 = empty_audio_unit();
    let mut output2 = empty_audio_unit();
    std::thread::sleep(std::time::Duration::from_millis(50));
    convolver.process(Some(&input2), &mut output2);

    // 63 + 5 = 68. 68 - 64 = 4
    assert!((output2[4][0] - 0.1).abs() < 1e-5);
    assert!((output2[4][1] - 0.1).abs() < 1e-5);

    for i in 0..AUDIO_UNIT_SIZE {
        if i != 4 {
            assert!((output2[i][0]).abs() < 1e-5);
        }
    }
}
