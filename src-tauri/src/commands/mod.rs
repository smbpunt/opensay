use tauri::State;

use crate::app::AppController;
use crate::domain::AppConfig;

/// Get the current application configuration.
#[tauri::command]
pub fn get_config(controller: State<'_, AppController>) -> Result<AppConfig, String> {
    Ok(controller.config())
}

/// Update the application configuration.
#[tauri::command]
pub fn update_config(
    controller: State<'_, AppController>,
    config: AppConfig,
) -> Result<(), String> {
    controller
        .update_config(config)
        .map_err(|e| e.to_string())
}

/// Check if network requests are currently blocked.
#[tauri::command]
pub fn is_network_blocked(controller: State<'_, AppController>) -> bool {
    controller.is_network_blocked()
}

/// Get application paths information.
#[tauri::command]
pub fn get_paths(controller: State<'_, AppController>) -> AppPaths {
    AppPaths {
        data_dir: controller.data_dir(),
        logs_dir: controller.logs_dir(),
        config_path: controller.config_path(),
    }
}

/// Application paths information.
#[derive(serde::Serialize)]
pub struct AppPaths {
    pub data_dir: String,
    pub logs_dir: String,
    pub config_path: String,
}
