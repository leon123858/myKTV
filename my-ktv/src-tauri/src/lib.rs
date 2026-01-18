use std::time::{Duration, Instant};

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use audio_core::AudioKernel;
use tauri::State;

use crate::audio_core::SendWrapper;

mod audio_core;
mod dsp;

#[tauri::command]
fn greet(name: &str, state: State<'_, SendWrapper<std::sync::Mutex<AudioKernel>>>) -> String {
    // 獲取 Producer 所有權 (通常會移交給解碼執行緒)
    let mut kernel = state.0.lock().expect("Mutex poisoned");
    match kernel.audio_producer.push(0.0) {
        Ok(_) => println!("Successfully pushed to audio producer"),
        Err(_) => println!("Buffer is full"),
    }
    let mut phase = 0.0f32;
    let sample_rate = 48000.0;
    let freq = 440.0; // A4 Note

    let start_time = Instant::now();
    let duration_limit = Duration::from_secs(10);

    loop {
        if start_time.elapsed() >= duration_limit {
            println!("已經執行滿 10 秒，停止生成。");
            break;
        }
        // 模擬解碼塊：每次產生 1024 個 samples
        // 只要 Ring Buffer 沒滿，就一直塞
        while !kernel.audio_producer.is_full() {
            if start_time.elapsed() >= duration_limit {
                println!("已經執行滿 10 秒，停止生成。");
                break;
            }
            // 生成立體聲數據
            let val = (phase * 2.0 * std::f32::consts::PI).sin() * 0.5;

            // 嘗試推入兩個 sample (L/R)
            // push 是一個極低成本的操作
            if kernel.audio_producer.push(val).is_err() {
                break;
            } // L
            if kernel.audio_producer.push(val).is_err() {
                break;
            } // R

            phase = (phase + freq / sample_rate) % 1.0;
        }

        // 模擬 I/O 等待 (避免佔用 100% CPU)
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    return format!("Hello, {}! You've been greeted from Rust!", name);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let audio_kernel = audio_core::AudioKernel::init()
        .expect("Failed to initialize audio kernel: check if output device is available");

    tauri::Builder::default()
        .manage(SendWrapper(std::sync::Mutex::new(audio_kernel)))
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
