// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use rust_audio_api::nodes::{
    ConvolverNode, DelayNode, FileNode, FilterNode, FilterType, GainNode, MicrophoneNode,
    MixerNode, NodeType,
};
use rust_audio_api::types::AUDIO_UNIT_SIZE;
use rust_audio_api::AudioContext;
use serde::Serialize;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;
use tauri::Emitter;
use tauri::Manager;
use tauri::State;
use tauri_plugin_shell::process::CommandEvent;
use tauri_plugin_shell::ShellExt;

// Audio state to manage playback
pub struct AudioState {
    context: Option<AudioContext>,
    current_file: Option<String>,
}

impl AudioState {
    fn new() -> Self {
        Self {
            context: None,
            current_file: None,
        }
    }
}

/// Generates a synthetic IR suitable for Asian KTV style (large room / hall reverb)
fn generate_karaoke_ir(sample_rate: u32) -> Vec<[f32; 2]> {
    let duration_sec = 1.2; // 1.2s reverb tail typical for KTV
    let len = (sample_rate as f32 * duration_sec) as usize;
    let mut ir = vec![[0.0f32; 2]; len];

    let mut seed: u32 = 12345;
    let mut rand_f32 = || -> f32 {
        seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
        ((seed >> 16) as f32 / 32768.0) - 1.0
    };

    let early_reflections: &[(usize, f32)] = &[
        (0, 1.0),
        ((0.015 * sample_rate as f64) as usize, 0.7),
        ((0.025 * sample_rate as f64) as usize, 0.5),
        ((0.040 * sample_rate as f64) as usize, 0.3),
        ((0.060 * sample_rate as f64) as usize, 0.2),
    ];

    for &(offset, gain) in early_reflections {
        if offset < len {
            ir[offset] = [gain, gain];
        }
    }

    let tail_start = (0.010 * sample_rate as f64) as usize;
    let decay_rate = 6.0 / duration_sec as f64;

    for (i, ir) in ir.iter_mut().enumerate().take(len).skip(tail_start) {
        let t = i as f64 / sample_rate as f64;
        let envelope = (-decay_rate * t).exp() as f32 * 0.4;
        ir[0] += rand_f32() * envelope;
        ir[1] += rand_f32() * envelope;
    }

    ir
}

#[tauri::command]
fn open_app_dir(app: tauri::AppHandle) {
    // 獲取 App 的資料儲存目錄 (例如: AppData/Roaming/<app-name>)
    if let Ok(path) = app.path().app_data_dir() {
        let path_str = path.to_string_lossy().to_string();

        #[cfg(target_os = "windows")]
        {
            Command::new("explorer").arg(path_str).spawn().unwrap();
        }

        #[cfg(target_os = "macos")]
        {
            Command::new("open").arg(path_str).spawn().unwrap();
        }

        #[cfg(target_os = "linux")]
        {
            Command::new("xdg-open").arg(path_str).spawn().unwrap();
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
    state.context = None;

    if path.is_empty() {
        return Err("File path is empty".to_string());
    }

    let file_path = PathBuf::from(&path);
    if !file_path.is_file() {
        return Err(format!("File not found or is a directory: {}", path));
    }

    let mut ctx = AudioContext::new().map_err(|e| e.to_string())?;
    let sample_rate = ctx.sample_rate();

    let dest_id = ctx.build_graph(|builder| {
        let file_node =
            FileNode::new(path.as_str(), sample_rate).expect("Unable to read audio file");
        let file = builder.add_node(NodeType::File(file_node));

        let mixer = builder.add_node(NodeType::Mixer(MixerNode::with_gain(1.0)));
        builder.connect(file, mixer);

        mixer
    });

    ctx.resume(dest_id).map_err(|e| e.to_string())?;

    state.context = Some(ctx);
    state.current_file = Some(path.clone());

    Ok(format!("Playing: {}", path))
}

#[tauri::command]
fn stop_audio(audio_state: State<'_, Mutex<AudioState>>) -> Result<String, String> {
    let mut state = audio_state.lock().map_err(|e| e.to_string())?;
    state.context = None;
    println!("[Stop] Stopped playback");
    Ok("Playback stopped".to_string())
}

#[tauri::command]
fn get_current_file(audio_state: State<'_, Mutex<AudioState>>) -> Result<String, String> {
    let state = audio_state.lock().map_err(|e| e.to_string())?;

    match &state.current_file {
        Some(path) => Ok(path.clone()),
        None => Ok("No file loaded".to_string()),
    }
}

use rust_audio_api::graph::{GraphBuilder, NodeId};

fn build_ktv_graph(
    builder: &mut GraphBuilder,
    sample_rate: u32,
    music_path: Option<&str>,
) -> NodeId {
    // Mic Setup
    let mic_node = MicrophoneNode::new(sample_rate).expect("Unable to open microphone");
    let mic = builder.add_node(NodeType::Microphone(mic_node));

    // Anti-howling filters (HighPass 200Hz + LowPass 6000Hz)
    let hp = builder.add_node(NodeType::Filter(FilterNode::new(
        FilterType::HighPass,
        sample_rate,
        200.0,
        0.707,
    )));
    builder.connect(mic, hp);

    let lp_mic = builder.add_node(NodeType::Filter(FilterNode::new(
        FilterType::LowPass,
        sample_rate,
        1000.0,
        0.707,
    )));
    builder.connect(hp, lp_mic);

    let mic_gain = builder.add_node(NodeType::Gain(GainNode::new(0.85)));
    builder.connect(lp_mic, mic_gain);

    let dry_gain = builder.add_node(NodeType::Gain(GainNode::new(0.8)));
    builder.connect(mic_gain, dry_gain);

    // Echo
    let delay_time_sec = 0.12;
    let delay_frames = (sample_rate as f32 * delay_time_sec) as usize;
    let delay_units = delay_frames / AUDIO_UNIT_SIZE;
    let max_delay_units = (sample_rate as usize * 2) / AUDIO_UNIT_SIZE;

    let delay = builder.add_node(NodeType::Delay(DelayNode::new(
        max_delay_units,
        delay_units,
    )));
    let echo_gain = builder.add_node(NodeType::Gain(GainNode::new(0.4)));
    let lp = builder.add_node(NodeType::Filter(FilterNode::new(
        FilterType::LowPass,
        sample_rate,
        1000.0,
        0.707,
    )));

    builder.connect(mic_gain, delay);
    builder.connect(delay, echo_gain);
    builder.connect_feedback(echo_gain, lp);
    builder.connect(lp, delay);

    // Reverb
    let ir = generate_karaoke_ir(sample_rate);
    let convolver_node = ConvolverNode::new(&ir);
    let convolver = builder.add_node(NodeType::Convolver(convolver_node));
    let reverb_lp = builder.add_node(NodeType::Filter(FilterNode::new(
        FilterType::LowPass,
        sample_rate,
        1000.0,
        0.707,
    )));
    let reverb_gain = builder.add_node(NodeType::Gain(GainNode::new(0.35)));

    builder.connect(mic_gain, convolver);
    builder.connect(convolver, reverb_lp);
    builder.connect(reverb_lp, reverb_gain);

    // Mix Output
    let mut mixer_node = MixerNode::with_gain(1.0);
    mixer_node.clipping = true;
    let mixer = builder.add_node(NodeType::Mixer(mixer_node));
    builder.connect(dry_gain, mixer);
    builder.connect(echo_gain, mixer);
    builder.connect(reverb_gain, mixer);

    // Music Setup (Optional)
    if let Some(path) = music_path {
        let file_node = FileNode::new(path, sample_rate).expect("Unable to read audio file");
        let file = builder.add_node(NodeType::File(file_node));
        let music_gain = builder.add_node(NodeType::Gain(GainNode::new(0.5)));
        let lp_music = builder.add_node(NodeType::Filter(FilterNode::new(
            FilterType::LowPass,
            sample_rate,
            1000.0,
            0.707,
        )));
        builder.connect(file, lp_music);
        builder.connect(lp_music, music_gain);
        builder.connect(music_gain, mixer);
    }

    mixer
}

#[tauri::command]
fn start_mic_only(audio_state: State<'_, Mutex<AudioState>>) -> Result<String, String> {
    let mut state = audio_state.lock().map_err(|e| e.to_string())?;

    println!("[Mic] Starting microphone only mode");

    state.context = None;

    let mut ctx = AudioContext::new().map_err(|e| e.to_string())?;
    let sample_rate = ctx.sample_rate();

    let dest_id = ctx.build_graph(|builder| build_ktv_graph(builder, sample_rate, None));

    ctx.resume(dest_id).map_err(|e| e.to_string())?;
    state.context = Some(ctx);

    Ok("Microphone started".to_string())
}

#[tauri::command]
fn stop_mic(audio_state: State<'_, Mutex<AudioState>>) -> Result<String, String> {
    let mut state = audio_state.lock().map_err(|e| e.to_string())?;
    state.context = None;
    println!("[Mic] Stopped microphone");
    Ok("Microphone stopped".to_string())
}

#[tauri::command]
fn start_karaoke(
    path: String,
    audio_state: State<'_, Mutex<AudioState>>,
) -> Result<String, String> {
    let mut state = audio_state.lock().map_err(|e| e.to_string())?;

    println!("[Karaoke] Starting karaoke mode with: {}", path);

    state.context = None;

    if path.is_empty() {
        return Err("Path is empty".to_string());
    }

    let file_path = PathBuf::from(&path);
    if !file_path.is_file() {
        return Err(format!("File not found or is not a file: {}", path));
    }

    let mut ctx = AudioContext::new().map_err(|e| e.to_string())?;
    let sample_rate = ctx.sample_rate();

    let dest_id =
        ctx.build_graph(|builder| build_ktv_graph(builder, sample_rate, Some(path.as_str())));

    ctx.resume(dest_id).map_err(|e| e.to_string())?;

    state.context = Some(ctx);
    state.current_file = Some(path.clone());

    Ok(format!("Karaoke started: {}", path))
}

#[tauri::command]
fn stop_karaoke(audio_state: State<'_, Mutex<AudioState>>) -> Result<String, String> {
    let mut state = audio_state.lock().map_err(|e| e.to_string())?;
    state.context = None;
    state.current_file = None;
    println!("[Karaoke] Stopped karaoke mode");
    Ok("Karaoke stopped".to_string())
}

#[derive(Serialize)]
struct Song {
    name: String,
    video_path: String,
    audio_path: String,
}

#[tauri::command]
async fn download_youtube(app: tauri::AppHandle, url: String) -> Result<(), String> {
    let output_dir = app.path().app_data_dir().unwrap().join("downloads");
    std::fs::create_dir_all(&output_dir).map_err(|e| e.to_string())?;

    let sidecar_command = app
        .shell()
        .sidecar("downloader")
        .unwrap()
        .args([url, output_dir.to_string_lossy().to_string()]);

    let (mut rx, _child) = sidecar_command.spawn().map_err(|e| e.to_string())?;

    tauri::async_runtime::spawn(async move {
        while let Some(event) = rx.recv().await {
            if let CommandEvent::Stdout(line) = event {
                if let Ok(line_str) = String::from_utf8(line) {
                    println!("Downloader: {}", line_str);
                    let _ = app.emit("download-progress", line_str);
                }
            } else if let CommandEvent::Stderr(line) = event {
                if let Ok(line_str) = String::from_utf8(line) {
                    println!("Downloader Err: {}", line_str);
                }
            }
        }
    });

    Ok(())
}

#[tauri::command]
fn get_downloaded_songs(app: tauri::AppHandle) -> Result<Vec<Song>, String> {
    let output_dir = app.path().app_data_dir().unwrap().join("downloads");
    if !output_dir.exists() {
        return Ok(Vec::new());
    }

    let data_dir = output_dir.join("data");

    let mut songs = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&output_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("mp4") {
                let name = path.file_stem().unwrap().to_string_lossy().to_string();

                let video_path = data_dir.join(format!("{}_video.mp4", name));
                let audio_path = data_dir.join(format!("{}_audio.mp3", name));

                let v_path = if video_path.exists() {
                    video_path.to_string_lossy().to_string()
                } else {
                    String::new()
                };
                let a_path = if audio_path.exists() {
                    audio_path.to_string_lossy().to_string()
                } else {
                    String::new()
                };

                if v_path == "" || a_path == "" {
                    return Err("data path not exist".to_string());
                }

                songs.push(Song {
                    name,
                    video_path: v_path,
                    audio_path: a_path,
                });
            }
        }
    }
    Ok(songs)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
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
            stop_karaoke,
            download_youtube,
            get_downloaded_songs,
            open_app_dir
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
