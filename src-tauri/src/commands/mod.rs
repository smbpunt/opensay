use std::path::PathBuf;

use tauri::State;

use crate::app::{AppController, ToggleResult};
use crate::domain::{
    AppConfig, AudioConfig, AudioDevice, AudioState, HardwareProfile, InstalledModel,
    ModelCatalog, ModelRecommendation, Quantization,
};
use crate::ports::{TranscribeConfig, TranscriptionResult};

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

/// Toggle recording: start if idle, stop + transcribe + inject if recording.
///
/// This is the main entry point for the global shortcut flow (Option+Space).
/// Returns the result indicating whether recording started or completed with text.
#[tauri::command]
pub async fn toggle_recording(controller: State<'_, AppController>) -> Result<ToggleResult, String> {
    controller
        .toggle_recording()
        .await
        .map_err(|e| e.to_string())
}

// ==================== Transcription Commands ====================

/// Transcribe recorded audio.
/// This starts recording, waits for stop, then transcribes.
#[tauri::command]
pub async fn transcribe(
    controller: State<'_, AppController>,
    language: Option<String>,
) -> Result<TranscriptionResult, String> {
    // Stop recording and get buffer
    let buffer = controller
        .stop_recording()
        .await
        .map_err(|e| e.to_string())?;

    // Transcribe
    let config = TranscribeConfig {
        language,
        ..Default::default()
    };

    controller
        .transcribe(buffer, Some(config))
        .await
        .map_err(|e| e.to_string())
}

/// Load a transcription model.
#[tauri::command]
pub async fn load_model(
    controller: State<'_, AppController>,
    path: String,
) -> Result<(), String> {
    controller
        .load_model(PathBuf::from(path))
        .await
        .map_err(|e| e.to_string())
}

/// Load a model by ID (uses installed model path).
#[tauri::command]
pub async fn load_model_by_id(
    controller: State<'_, AppController>,
    model_id: String,
    quantization: String,
) -> Result<(), String> {
    let quant = Quantization::from_suffix(&quantization)
        .ok_or_else(|| format!("Invalid quantization: {}", quantization))?;

    let path = controller
        .model_path(&model_id, quant)
        .ok_or_else(|| format!("Model not installed: {}-{}", model_id, quantization))?;

    controller
        .load_model(path)
        .await
        .map_err(|e| e.to_string())
}

/// Check if a model is loaded.
#[tauri::command]
pub fn is_model_loaded(controller: State<'_, AppController>) -> bool {
    controller.is_model_loaded()
}

/// Unload the current model.
#[tauri::command]
pub fn unload_model(controller: State<'_, AppController>) {
    controller.unload_model();
}

// ==================== Model Management Commands ====================

/// Get the model catalog.
#[tauri::command]
pub fn get_model_catalog(controller: State<'_, AppController>) -> ModelCatalog {
    controller.model_catalog()
}

/// List installed models.
#[tauri::command]
pub fn list_installed_models(
    controller: State<'_, AppController>,
) -> Result<Vec<InstalledModel>, String> {
    controller
        .list_installed_models()
        .map_err(|e| e.to_string())
}

/// Check if a model is installed.
#[tauri::command]
pub fn is_model_installed(
    controller: State<'_, AppController>,
    model_id: String,
    quantization: String,
) -> Result<bool, String> {
    let quant = Quantization::from_suffix(&quantization)
        .ok_or_else(|| format!("Invalid quantization: {}", quantization))?;

    Ok(controller.is_model_installed(&model_id, quant))
}

/// Download a model.
#[tauri::command]
pub async fn download_model(
    controller: State<'_, AppController>,
    model_id: String,
    quantization: String,
) -> Result<InstalledModel, String> {
    let quant = Quantization::from_suffix(&quantization)
        .ok_or_else(|| format!("Invalid quantization: {}", quantization))?;

    controller
        .download_model(&model_id, quant, None)
        .await
        .map_err(|e| e.to_string())
}

/// Delete an installed model.
#[tauri::command]
pub fn delete_model(
    controller: State<'_, AppController>,
    model_id: String,
    quantization: String,
) -> Result<(), String> {
    let quant = Quantization::from_suffix(&quantization)
        .ok_or_else(|| format!("Invalid quantization: {}", quantization))?;

    controller
        .delete_model(&model_id, quant)
        .map_err(|e| e.to_string())
}

/// Get the models directory path.
#[tauri::command]
pub fn get_models_dir(controller: State<'_, AppController>) -> String {
    controller.models_dir().to_string_lossy().to_string()
}

// ==================== Hardware Commands ====================

/// Get the hardware profile.
#[tauri::command]
pub fn get_hardware_profile(
    controller: State<'_, AppController>,
) -> Result<HardwareProfile, String> {
    controller.hardware_profile().map_err(|e| e.to_string())
}

/// Get the recommended model for this hardware.
#[tauri::command]
pub fn get_recommended_model(
    controller: State<'_, AppController>,
) -> Result<ModelRecommendation, String> {
    controller.recommended_model().map_err(|e| e.to_string())
}
