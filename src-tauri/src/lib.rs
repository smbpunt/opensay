#![forbid(unsafe_code)]

mod adapters;
mod app;
mod commands;
mod domain;
mod infrastructure;
mod ports;

use app::AppController;
use commands::{
    // Config commands
    get_config, update_config, is_network_blocked, get_paths,
    // Audio commands
    get_audio_config, get_audio_level, get_audio_state, get_recording_duration,
    list_audio_devices, recover_audio, select_audio_device, start_recording, stop_recording,
    toggle_recording,
    // Transcription commands
    transcribe, load_model, load_model_by_id, is_model_loaded, unload_model,
    // Model management commands
    get_model_catalog, list_installed_models, is_model_installed, download_model,
    delete_model, get_models_dir,
    // Hardware commands
    get_hardware_profile, get_recommended_model,
};
use tauri::Emitter;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

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
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        // Emit event to frontend when shortcut is triggered
                        if let Err(e) = app.emit("shortcut-triggered", shortcut.id()) {
                            tracing::error!("Failed to emit shortcut event: {}", e);
                        }
                    }
                })
                .build(),
        )
        .manage(controller)
        .setup(|app| {
            // Register Alt+Space global shortcut
            // NOTE: Shortcut is hardcoded; config.shortcut.toggle_shortcut is not parsed yet.
            // Parsing arbitrary shortcut strings requires a custom parser (future work).
            let shortcut = Shortcut::new(Some(Modifiers::ALT), Code::Space);
            if let Err(e) = app.global_shortcut().register(shortcut) {
                tracing::warn!("Failed to register global shortcut: {}", e);
            } else {
                tracing::info!("Global shortcut Alt+Space registered");
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Config commands
            get_config,
            update_config,
            is_network_blocked,
            get_paths,
            // Audio commands
            start_recording,
            stop_recording,
            toggle_recording,
            get_audio_state,
            get_audio_config,
            list_audio_devices,
            select_audio_device,
            get_recording_duration,
            get_audio_level,
            recover_audio,
            // Transcription commands
            transcribe,
            load_model,
            load_model_by_id,
            is_model_loaded,
            unload_model,
            // Model management commands
            get_model_catalog,
            list_installed_models,
            is_model_installed,
            download_model,
            delete_model,
            get_models_dir,
            // Hardware commands
            get_hardware_profile,
            get_recommended_model,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
