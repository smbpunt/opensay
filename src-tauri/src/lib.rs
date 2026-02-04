#![forbid(unsafe_code)]

mod adapters;
mod app;
mod commands;
mod domain;
mod infrastructure;
mod ports;

use app::AppController;
use commands::{get_config, get_paths, is_network_blocked, update_config};

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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
