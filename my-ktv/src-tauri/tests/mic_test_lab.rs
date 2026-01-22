use my_ktv_lib::audio_node::{
    connect, mic_src::MicSrc, speaker_dest::SpeakerDest, AudioNode, AudioNodeEnum, AudioNodeState,
};
use std::thread;
use std::time::Duration;
use cpal::traits::{DeviceTrait, HostTrait};

// 輔助函式：快速建立並連接
fn setup_loopback() -> (AudioNodeEnum, AudioNodeEnum) {
    let mut mic = AudioNodeEnum::MicSrc(MicSrc::init());
    let mut spk = AudioNodeEnum::SpeakerDest(SpeakerDest::init());

    // 測試連接邏輯
    connect(&mut mic, &mut spk).expect("連接失敗");

    (mic, spk)
}

#[test]
fn test_all_mic_config() {
    let host = cpal::default_host();
    let input_device = host.default_input_device().expect("no input device");
    let supported_configs: Vec<_> = input_device
        .supported_output_configs()
        .expect("no supported config")
        .collect();
    for config in supported_configs {
        println!("[HAL] Supported Config: {:?}", config);
    }
    
}

#[test]
// 1. 純邏輯測試：確認 Producer 所有權有轉移
// 執行指令: cargo test test_connection_logic
fn test_connection_logic() {
    let (mic, spk) = setup_loopback();

    // 驗證狀態
    match mic {
        AudioNodeEnum::MicSrc(inner) => {
            assert!(inner.audio_producer.is_some(), "Mic 應該拿到 Producer");
        }
        _ => panic!("Wrong type"),
    }

    match spk {
        AudioNodeEnum::SpeakerDest(inner) => {
            assert!(inner.audio_producer.is_none(), "Speaker 應該交出 Producer");
        }
        _ => panic!("Wrong type"),
    }
}

#[test]
#[ignore] // 加上 ignore，避免自動測試時執行
          // 2. 聽感測試：預設設定
          // 執行指令: cargo test test_mic_sound -- --nocapture --ignored
fn test_mic_sound_default() {
    println!("=== 測試開始：預設參數 (請對麥克風說話) ===");
    let (mut mic, mut spk) = setup_loopback();

    spk.start();
    mic.start(); // 這裡會印出 MicSrc 的參數 Log

    // 讓它跑 5 秒鐘
    thread::sleep(Duration::from_secs(5));

    mic.stop();
    spk.stop();
    println!("=== 測試結束 ===");
}

#[test]
#[ignore]
// 3. 聽感測試：壓力測試 (快速開關)
// 執行指令: cargo test test_mic_stress -- --nocapture --ignored
fn test_mic_stress() {
    println!("=== 測試開始：壓力測試 (快速開關) ===");
    let (mut mic, mut spk) = setup_loopback();

    for i in 1..=3 {
        println!("Loop #{}", i);
        spk.start();
        mic.start();
        thread::sleep(Duration::from_millis(500)); // 跑 0.5 秒
        mic.stop(); // 停止
        spk.stop();
        thread::sleep(Duration::from_millis(200)); // 休息
    }
    println!("=== 測試結束 ===");
}
