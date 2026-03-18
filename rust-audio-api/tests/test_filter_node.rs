use rust_audio_api::nodes::{FilterNode, FilterType};
use rust_audio_api::types::{AUDIO_UNIT_SIZE, empty_audio_unit};

#[test]
fn test_filter_node_initialization() {
    // 驗證所有三種濾波器類型都能順利建立
    let _lp = FilterNode::new(FilterType::LowPass, 48000, 1000.0, 0.707);
    let _hp = FilterNode::new(FilterType::HighPass, 48000, 1000.0, 0.707);
    let _bp = FilterNode::new(FilterType::BandPass, 48000, 1000.0, 0.707);
}

#[test]
fn test_lowpass_passes_dc() {
    // DC 信號（常數值）應該能完全通過低通濾波器
    let mut filter = FilterNode::new(FilterType::LowPass, 48000, 1000.0, 0.707);
    let mut input = empty_audio_unit();
    for i in 0..AUDIO_UNIT_SIZE {
        input[i] = [0.5, 0.5];
    }

    // 多跑幾輪讓濾波器穩態收斂
    let mut output = empty_audio_unit();
    for _ in 0..100 {
        filter.process(Some(&input), &mut output);
    }

    // 穩態後最後一個 sample 應該非常接近 DC 輸入值
    let last = output[AUDIO_UNIT_SIZE - 1];
    assert!(
        (last[0] - 0.5).abs() < 0.01,
        "LowPass DC pass-through failed: got L={}, expected ~0.5",
        last[0]
    );
    assert!(
        (last[1] - 0.5).abs() < 0.01,
        "LowPass DC pass-through failed: got R={}, expected ~0.5",
        last[1]
    );
}

#[test]
fn test_highpass_rejects_dc() {
    // DC 信號應該被高通濾波器完全濾掉（衰減到接近 0）
    let mut filter = FilterNode::new(FilterType::HighPass, 48000, 1000.0, 0.707);
    let mut input = empty_audio_unit();
    for i in 0..AUDIO_UNIT_SIZE {
        input[i] = [0.5, 0.5];
    }

    // 多跑幾輪讓濾波器穩態收斂
    let mut output = empty_audio_unit();
    for _ in 0..100 {
        filter.process(Some(&input), &mut output);
    }

    // 穩態後最後一個 sample 應該非常接近 0
    let last = output[AUDIO_UNIT_SIZE - 1];
    assert!(
        last[0].abs() < 0.01,
        "HighPass DC rejection failed: got L={}, expected ~0.0",
        last[0]
    );
    assert!(
        last[1].abs() < 0.01,
        "HighPass DC rejection failed: got R={}, expected ~0.0",
        last[1]
    );
}

#[test]
fn test_filter_no_input_outputs_silence() {
    let mut filter = FilterNode::new(FilterType::LowPass, 48000, 1000.0, 0.707);
    let mut output = empty_audio_unit();

    // 先把 output 填上非零值，確認 process(None) 會清零
    for i in 0..AUDIO_UNIT_SIZE {
        output[i] = [1.0, 1.0];
    }

    filter.process(None, &mut output);

    let expected = empty_audio_unit();
    assert_eq!(
        output, expected,
        "Filter with no input should output silence"
    );
}

#[test]
fn test_filter_set_parameters_no_panic() {
    let mut filter = FilterNode::new(FilterType::LowPass, 48000, 1000.0, 0.707);
    let mut output = empty_audio_unit();
    let input = empty_audio_unit();

    // 動態修改參數不應 panic
    filter.set_cutoff(5000.0);
    filter.process(Some(&input), &mut output);

    filter.set_q(2.0);
    filter.process(Some(&input), &mut output);

    filter.set_filter_type(FilterType::HighPass);
    filter.process(Some(&input), &mut output);

    filter.set_filter_type(FilterType::BandPass);
    filter.process(Some(&input), &mut output);
}
