// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use crate::audio_node::file_src::FileSrc;
use crate::audio_node::speaker_dest::SpeakerDest;
use crate::audio_node::{connect, AudioNode, AudioNodeEnum};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::State;

pub mod audio_node;

pub struct SendWrapper<T>(pub T);
unsafe impl<T> Send for SendWrapper<T> {}
unsafe impl<T> Sync for SendWrapper<T> {}

// Audio state to manage playback
pub struct AudioState {
    file_src: Option<AudioNodeEnum>,
    speaker_dest: Option<AudioNodeEnum>,
    current_file: Option<String>,
}

impl AudioState {
    fn new() -> Self {
        Self {
            file_src: None,
            speaker_dest: Some(AudioNodeEnum::SpeakerDest(SpeakerDest::init())),
            current_file: None,
        }
    }
}

#[tauri::command]
async fn upload_audio_file(app: tauri::AppHandle) -> Result<String, String> {
    use tauri_plugin_dialog::DialogExt;

    // Open file picker dialog
    let file_path = app
        .dialog()
        .file()
        .add_filter("Audio Files", &["mp3", "wav", "flac", "ogg", "m4a"])
        .blocking_pick_file();

    match file_path {
        Some(path) => {
            let path_str = path.simplified().to_string();
            println!("[Upload] Selected file: {}", path_str);
            Ok(path_str)
        }
        None => Err("No file selected".to_string()),
    }
}

#[tauri::command]
fn play_audio_file(
    path: String,
    audio_state: State<'_, Mutex<AudioState>>,
) -> Result<String, String> {
    let mut state = audio_state.lock().map_err(|e| e.to_string())?;

    println!("[Play] Attempting to play: {}", path);

    // Stop any existing playback
    if let Some(ref mut src) = state.file_src {
        src.stop();
    }

    // Create new FileSrc with the selected file
    let file_path = PathBuf::from(&path);
    if !file_path.exists() {
        return Err(format!("File not found: {}", path));
    }

    let file_src = FileSrc::new(file_path, 48000, 2);
    let mut src_node = AudioNodeEnum::FileSrc(file_src);

    // Connect to speaker if available
    if let Some(ref mut dest) = state.speaker_dest {
        // Start speaker first if not running
        if !matches!(dest.get_state(), crate::audio_node::AudioNodeState::RUNNING) {
            dest.start();
            println!("[Play] Started speaker");
        }

        // Connect source to destination
        connect(&mut src_node, dest).map_err(|e| format!("Connection failed: {}", e))?;
        println!("[Play] Connected file source to speaker");
    } else {
        return Err("Speaker not available".to_string());
    }

    // Start playback
    src_node.start();
    println!("[Play] Started playback");

    state.file_src = Some(src_node);
    state.current_file = Some(path.clone());

    Ok(format!("Playing: {}", path))
}

#[tauri::command]
fn stop_audio(audio_state: State<'_, Mutex<AudioState>>) -> Result<String, String> {
    let mut state = audio_state.lock().map_err(|e| e.to_string())?;

    if let Some(ref mut src) = state.file_src {
        src.stop();
        println!("[Stop] Stopped playback");
        Ok("Playback stopped".to_string())
    } else {
        Err("No audio playing".to_string())
    }
}

#[tauri::command]
fn get_current_file(audio_state: State<'_, Mutex<AudioState>>) -> Result<String, String> {
    let state = audio_state.lock().map_err(|e| e.to_string())?;

    match &state.current_file {
        Some(path) => Ok(path.clone()),
        None => Ok("No file loaded".to_string()),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(Mutex::new(AudioState::new()))
        .invoke_handler(tauri::generate_handler![
            upload_audio_file,
            play_audio_file,
            stop_audio,
            get_current_file
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
