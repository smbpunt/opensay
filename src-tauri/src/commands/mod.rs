use tauri::State;

use crate::app::AppController;
use crate::domain::{AppConfig, AudioConfig, AudioDevice, AudioState};

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

// ==================== Audio Commands ====================

/// Start audio recording.
#[tauri::command]
pub async fn start_recording(controller: State<'_, AppController>) -> Result<(), String> {
    controller
        .start_recording()
        .await
        .map_err(|e| e.to_string())
}

/// Stop audio recording and return duration.
#[tauri::command]
pub async fn stop_recording(controller: State<'_, AppController>) -> Result<RecordingResult, String> {
    let buffer = controller
        .stop_recording()
        .await
        .map_err(|e| e.to_string())?;

    Ok(RecordingResult {
        duration_secs: buffer.duration_secs(),
        sample_count: buffer.len(),
    })
}

/// Result of a recording session.
#[derive(serde::Serialize)]
pub struct RecordingResult {
    pub duration_secs: f32,
    pub sample_count: usize,
}

/// Get current audio state.
#[tauri::command]
pub fn get_audio_state(controller: State<'_, AppController>) -> AudioState {
    controller.audio_state()
}

/// Get audio configuration.
#[tauri::command]
pub fn get_audio_config(controller: State<'_, AppController>) -> AudioConfig {
    controller.audio_config()
}

/// List available audio input devices.
#[tauri::command]
pub fn list_audio_devices(controller: State<'_, AppController>) -> Result<Vec<AudioDevice>, String> {
    controller
        .list_audio_devices()
        .map_err(|e| e.to_string())
}

/// Select an audio input device.
#[tauri::command]
pub fn select_audio_device(
    controller: State<'_, AppController>,
    device_id: Option<String>,
) -> Result<(), String> {
    controller
        .select_audio_device(device_id.as_deref())
        .map_err(|e| e.to_string())
}

/// Get current recording duration.
#[tauri::command]
pub fn get_recording_duration(controller: State<'_, AppController>) -> f32 {
    controller.recording_duration()
}

/// Get current audio input level.
#[tauri::command]
pub fn get_audio_level(controller: State<'_, AppController>) -> f32 {
    controller.audio_level()
}

/// Attempt to recover from audio error state.
#[tauri::command]
pub async fn recover_audio(controller: State<'_, AppController>) -> Result<(), String> {
    controller
        .recover_audio()
        .await
        .map_err(|e| e.to_string())
}
