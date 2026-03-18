use rust_audio_api::nodes::FileNode;
use rust_audio_api::types::empty_audio_unit;
use std::path::Path;
use std::time::Duration;
use std::{env, f32, thread};

/// Helper to generate a dummy WAV file for testing
fn create_dummy_wav(file_path: &Path) {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: 44100,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(file_path, spec).unwrap();

    // 撰寫半秒鐘的 440Hz 聲音
    for t in 0..(44100 / 2) {
        let sample = (t as f32 * 440.0 * 2.0 * f32::consts::PI / 44100.0).sin();
        let amplitude = i16::MAX as f32;
        writer.write_sample((sample * amplitude) as i16).unwrap(); // Left
        writer.write_sample((sample * amplitude) as i16).unwrap(); // Right
    }
    writer.finalize().unwrap();
}

#[test]
fn test_file_node_initialization() {
    let mut temp_path = env::temp_dir();
    temp_path.push("test_initialization.wav");
    create_dummy_wav(&temp_path);

    let file_node_result = FileNode::new(temp_path.to_str().unwrap(), 48000);
    assert!(file_node_result.is_ok());

    std::fs::remove_file(temp_path).unwrap();
}

#[test]
fn test_file_node_set_gain() {
    let mut temp_path = env::temp_dir();
    temp_path.push("test_gain.wav");
    create_dummy_wav(&temp_path);

    let mut file_node = FileNode::new(temp_path.to_str().unwrap(), 48000).unwrap();
    let mut output = empty_audio_unit();

    thread::sleep(Duration::from_millis(50));

    file_node.set_gain(0.5);
    file_node.process(None, &mut output);

    // Verify amplitude scaling bound (approx)
    let mut has_non_zero = false;
    for frame in output.iter() {
        if frame[0] != 0.0 {
            has_non_zero = true;
            assert!(frame[0].abs() <= 0.51); // 0.5 + epsilon margin
        }
    }
    assert!(has_non_zero, "File output stream was empty");

    std::fs::remove_file(temp_path).unwrap();
}

#[test]
fn test_file_node_process_reads_audio() {
    let mut temp_path = env::temp_dir();
    temp_path.push("test_process.wav");
    create_dummy_wav(&temp_path);

    // target sample rate (48k) != file rate (44.1k) => triggering resampler branch
    let mut file_node = FileNode::new(temp_path.to_str().unwrap(), 48000).unwrap();
    let mut output = empty_audio_unit();

    thread::sleep(Duration::from_millis(50));

    file_node.process(None, &mut output);

    let mut has_non_zero = false;
    for frame in output.iter() {
        if frame[0] != 0.0 || frame[1] != 0.0 {
            has_non_zero = true;
        }
    }

    assert!(has_non_zero, "Resampled file data was purely empty");

    std::fs::remove_file(temp_path).unwrap();
}
