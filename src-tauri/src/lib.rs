#![forbid(unsafe_code)]

mod adapters;
mod app;
mod commands;
mod domain;
mod infrastructure;
mod ports;

use app::AppController;
use commands::{
    get_audio_config, get_audio_level, get_audio_state, get_config, get_paths,
    get_recording_duration, is_network_blocked, list_audio_devices, recover_audio,
    select_audio_device, start_recording, stop_recording, update_config,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize the application controller
    let controller = match AppController::new() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to initialize application: {}", e);
            std::process::exit(1);
        }
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(controller)
        .invoke_handler(tauri::generate_handler![
            get_config,
            update_config,
            is_network_blocked,
            get_paths,
            // Audio commands
            start_recording,
            stop_recording,
            get_audio_state,
            get_audio_config,
            list_audio_devices,
            select_audio_device,
            get_recording_duration,
            get_audio_level,
            recover_audio,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
