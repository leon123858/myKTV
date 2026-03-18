// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use crate::audio_node::file_src::FileSrc;
use crate::audio_node::mic_src::MicSrc;
use crate::audio_node::mixer::Mixer;
use crate::audio_node::speaker_dest::SpeakerDest;
use crate::audio_node::{connect, AudioNode, AudioNodeEnum};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::State;

mod audio_api;
pub mod audio_node;

pub struct SendWrapper<T>(pub T);
unsafe impl<T> Send for SendWrapper<T> {}
unsafe impl<T> Sync for SendWrapper<T> {}

// Audio state to manage playback
pub struct AudioState {
    file_src: Option<AudioNodeEnum>,
    mic_src: Option<AudioNodeEnum>,
    mixer: Option<AudioNodeEnum>,
    speaker_dest: Option<AudioNodeEnum>,
    current_file: Option<String>,
}

impl AudioState {
    fn new() -> Self {
        Self {
            file_src: None,
            mic_src: None,
            mixer: None,
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

    let mut src_node = FileSrc::init();

    // Connect to speaker if available
    if let Some(ref mut dest) = state.speaker_dest {
        // Start speaker first if not running
        if !matches!(dest.get_state(), crate::audio_node::AudioNodeState::RUNNING) {
            dest.start();
            println!("[Play] Started speaker");
        }

        // set src node config
        let dest_node = match dest {
            AudioNodeEnum::SpeakerDest(dest) => dest,
            _ => return Err("Speaker not available".to_string()),
        };
        src_node.set_config(
            file_path,
            dest_node.config.stream_config.sample_rate,
            dest_node.config.stream_config.channels.into(),
        );
        let mut src = AudioNodeEnum::FileSrc(src_node);

        // new mixer node (dest node buffer 太小，會掉資料，一定要墊一個 push node)
        let mixer = Mixer::new(0);
        let mut mixer_enum = AudioNodeEnum::Mixer(mixer);

        // Connect source to destination
        connect(&mut src, &mut mixer_enum).map_err(|e| format!("Connection failed: {}", e))?;
        connect(&mut mixer_enum, dest).map_err(|e| format!("Connection failed: {}", e))?;
        println!("[Play] Connected file source to speaker");

        mixer_enum.start();
        src.start();
        println!("[Play] Started playback");

        state.file_src = Some(src);
        state.current_file = Some(path.clone());

        Ok(format!("Playing: {}", path))
    } else {
        Err("Speaker not available".to_string())
    }
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

#[tauri::command]
fn start_mic_only(audio_state: State<'_, Mutex<AudioState>>) -> Result<String, String> {
    let mut state = audio_state.lock().map_err(|e| e.to_string())?;

    println!("[Mic] Starting microphone only mode");

    // Stop any existing microphone
    if let Some(ref mut mic) = state.mic_src {
        mic.stop();
    }
    if let Some(ref mut mixer) = state.mixer {
        mixer.stop();
    }

    // Start speaker if not running
    if let Some(ref mut dest) = state.speaker_dest {
        if !matches!(dest.get_state(), crate::audio_node::AudioNodeState::RUNNING) {
            dest.start();
            println!("[Mic] Started speaker");
        }

        // Get speaker config
        let dest_config = match dest {
            AudioNodeEnum::SpeakerDest(dest) => dest.config.clone(),
            _ => return Err("Speaker not available".to_string()),
        };

        // Create mic source
        let mut mic_src = MicSrc::init();
        mic_src.input_producer_config = Some(dest_config);

        // Wrap in enum
        let mut mic_src_enum = AudioNodeEnum::MicSrc(mic_src);

        // new mixer node
        let mixer = Mixer::new(0);
        let mut mixer_enum = AudioNodeEnum::Mixer(mixer);

        // Connect
        connect(&mut mic_src_enum, &mut mixer_enum)
            .map_err(|e| format!("Mic->Speaker connection failed: {}", e))?;
        connect(&mut mixer_enum, dest)
            .map_err(|e| format!("Mic->Speaker connection failed: {}", e))?;
        println!("[Mic] Connected microphone to speaker");

        // Start microphone
        mic_src_enum.start();
        println!("[Mic] Started microphone");
        mixer_enum.start();
        println!("[Mic] Started mixer");

        // Store in state
        state.mic_src = Some(mic_src_enum);

        Ok("Microphone started".to_string())
    } else {
        Err("Speaker not available".to_string())
    }
}

#[tauri::command]
fn stop_mic(audio_state: State<'_, Mutex<AudioState>>) -> Result<String, String> {
    let mut state = audio_state.lock().map_err(|e| e.to_string())?;

    println!("[Mic] Stopping microphone");

    if let Some(ref mut mic) = state.mic_src {
        mic.stop();
        state.mic_src = None;
        println!("[Mic] Stopped microphone");
        Ok("Microphone stopped".to_string())
    } else {
        Err("No microphone active".to_string())
    }
}

#[tauri::command]
fn start_karaoke(
    path: String,
    audio_state: State<'_, Mutex<AudioState>>,
) -> Result<String, String> {
    let mut state = audio_state.lock().map_err(|e| e.to_string())?;

    println!("[Karaoke] Starting karaoke mode with: {}", path);

    // Stop any existing playback
    if let Some(ref mut src) = state.file_src {
        src.stop();
    }
    if let Some(ref mut mic) = state.mic_src {
        mic.stop();
    }
    if let Some(ref mut mixer) = state.mixer {
        mixer.stop();
    }

    // Verify file exists
    let file_path = PathBuf::from(&path);
    if !file_path.exists() {
        return Err(format!("File not found: {}", path));
    }

    // Start speaker if not running
    if let Some(ref mut dest) = state.speaker_dest {
        if !matches!(dest.get_state(), crate::audio_node::AudioNodeState::RUNNING) {
            dest.start();
            println!("[Karaoke] Started speaker");
        }

        // Get speaker config and clone it to avoid borrowing issues
        let (sample_rate, channels, dest_config) = match dest {
            AudioNodeEnum::SpeakerDest(dest) => (
                dest.config.stream_config.sample_rate,
                dest.config.stream_config.channels,
                dest.config.clone(),
            ),
            _ => return Err("Speaker not available".to_string()),
        };

        let mixer = Mixer::new(0);

        let mut file_src = FileSrc::init();
        file_src.set_config(file_path, sample_rate, channels.into());

        let mut mic_src = MicSrc::init();
        mic_src.input_producer_config = Some(dest_config);

        let mut mixer_enum = AudioNodeEnum::Mixer(mixer);
        let mut file_src_enum = AudioNodeEnum::FileSrc(file_src);
        let mut mic_src_enum = AudioNodeEnum::MicSrc(mic_src);

        connect(&mut mixer_enum, dest)
            .map_err(|e| format!("Mixer->Speaker connection failed: {}", e))?;
        println!("[Karaoke] Connected mixer to speaker");

        // Connect: file_src -> mixer (input 0)
        connect(&mut file_src_enum, &mut mixer_enum)
            .map_err(|e| format!("File->Mixer connection failed: {}", e))?;
        println!("[Karaoke] Connected file source to mixer");

        // Connect: mic_src -> mixer (input 1)
        connect(&mut mic_src_enum, &mut mixer_enum)
            .map_err(|e| format!("Mic->Mixer connection failed: {}", e))?;
        println!("[Karaoke] Connected mic source to mixer");

        // Start all nodes
        mixer_enum.start();
        println!("[Karaoke] Started mixer");

        file_src_enum.start();
        println!("[Karaoke] Started file playback");

        mic_src_enum.start();
        println!("[Karaoke] Started microphone");

        // Store in state
        state.mixer = Some(mixer_enum);
        state.file_src = Some(file_src_enum);
        state.mic_src = Some(mic_src_enum);
        state.current_file = Some(path.clone());

        Ok(format!("Karaoke started: {}", path))
    } else {
        Err("Speaker not available".to_string())
    }
}

#[tauri::command]
fn stop_karaoke(audio_state: State<'_, Mutex<AudioState>>) -> Result<String, String> {
    let mut state = audio_state.lock().map_err(|e| e.to_string())?;

    println!("[Karaoke] Stopping karaoke mode");

    // Stop all nodes
    if let Some(ref mut mic) = state.mic_src {
        mic.stop();
        println!("[Karaoke] Stopped microphone");
    }

    if let Some(ref mut src) = state.file_src {
        src.stop();
        println!("[Karaoke] Stopped file playback");
    }

    if let Some(ref mut mixer) = state.mixer {
        mixer.stop();
        println!("[Karaoke] Stopped mixer");
    }

    // Clear state
    state.mic_src = None;
    state.file_src = None;
    state.mixer = None;
    state.current_file = None;

    Ok("Karaoke stopped".to_string())
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
            get_current_file,
            start_mic_only,
            stop_mic,
            start_karaoke,
            stop_karaoke
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
