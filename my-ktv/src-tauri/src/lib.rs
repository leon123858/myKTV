// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
mod audio_core;

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let audio_kernel = audio_core::AudioKernel::init()
        .expect("Failed to initialize audio kernel: check if output device is available");

    tauri::Builder::default()
        .manage(audio_kernel)
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
