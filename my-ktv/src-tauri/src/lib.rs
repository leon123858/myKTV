// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use crate::audio_node::fake_audio_wave_src::FakeAudioWaveSRC;
use crate::audio_node::mic_src::MicSrc;
use crate::audio_node::speaker_dest::SpeakerDest;
use crate::audio_node::{connect, AudioNode, AudioNodeEnum};
use tauri::State;

pub mod audio_node;

pub struct SendWrapper<T>(pub T);
unsafe impl<T> Send for SendWrapper<T> {}
unsafe impl<T> Sync for SendWrapper<T> {}

#[tauri::command]
fn greet(
    name: &str,
    nodes_state: State<'_, SendWrapper<std::sync::Mutex<Vec<AudioNodeEnum>>>>,
    mode_state: State<'_, SendWrapper<std::sync::Mutex<i32>>>,
) -> String {
    let mut nodes = nodes_state.0.lock().expect("Mutex poisoned");
    let mut mode_state = mode_state.0.lock().expect("Mutex poisoned");

    if *mode_state == 0 {
        if let Some(first_node) = nodes.get_mut(1) {
            first_node.start();
        }
        *mode_state = 1;
    } else {
        if let Some(first_node) = nodes.get_mut(1) {
            first_node.stop();
        }
        *mode_state = 0;
    }

    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut nodes: Vec<AudioNodeEnum> = vec![
        AudioNodeEnum::MicSrc(MicSrc::init()),
        AudioNodeEnum::SpeakerDest(SpeakerDest::init()),
    ];

    if let [src, dest] = &mut nodes[..2] {
        connect(src, dest).expect("Connect failed");
    }

    nodes[0].start();
    println!(
        "Node type: {:?}, State: {:?}",
        nodes[0].get_type(),
        nodes[0].get_state()
    );

    tauri::Builder::default()
        .manage(SendWrapper(std::sync::Mutex::new(0)))
        .manage(SendWrapper(std::sync::Mutex::new(nodes)))
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
