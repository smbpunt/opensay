use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use parking_lot::RwLock;
use tokio::sync::broadcast;
use tracing::info;
use tracing_appender::non_blocking::WorkerGuard;

use crate::adapters::{
    ClipboardOutputManager, CpalAudioManager, CpuHardwareDetector, LocalModelManager,
    PrivacyGuard, TomlConfigStore, WhisperCppTranscriber,
};
use crate::domain::{
    AppConfig, AudioBuffer, AudioConfig, AudioDevice, AudioEvent, AudioState, DomainError,
    DownloadProgress, HardwareProfile, InstalledModel, ModelCatalog, ModelRecommendation,
    Quantization,
};
use crate::infrastructure::init_logging;
use crate::ports::{
    AudioManager, ConfigStore, HardwareDetector, HttpClient, ModelManager, OutputManager,
    TranscribeConfig, Transcriber, TranscriptionResult,
};

/// Result of a toggle recording operation.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type")]
pub enum ToggleResult {
    /// Recording started.
    Started,
    /// Recording stopped and transcription completed.
    Completed {
        /// The transcribed text, or None if no speech was detected.
        text: Option<String>,
    },
}

/// Application controller that orchestrates initialization and manages global state.
pub struct AppController {
    config: RwLock<AppConfig>,
    config_store: Arc<TomlConfigStore>,
    audio_manager: Arc<CpalAudioManager>,
    transcriber: Arc<WhisperCppTranscriber>,
    model_manager: Arc<LocalModelManager>,
    hardware_detector: Arc<CpuHardwareDetector>,
    output_manager: Arc<ClipboardOutputManager>,
    /// Guard against concurrent toggle_recording calls (e.g., keyboard repeat)
    toggle_in_progress: AtomicBool,
    _log_guard: Option<WorkerGuard>,
}

impl AppController {
    /// Initialize the application controller.
    /// This sets up configuration, logging, and the privacy guard.
    pub fn new() -> Result<Self, DomainError> {
        // Step 1: Initialize config store
        let config_store = Arc::new(TomlConfigStore::new()?);

        // Step 2: Load configuration
        let config = config_store.load()?;

        // Step 3: Initialize logging
        let log_guard = init_logging(
            &config_store.logs_dir(),
            &config.logging.level,
            config.logging.file_logging,
        )?;

        info!("OpenSay starting up");

        // Step 4: Initialize PrivacyGuard with config settings
        let _ = PrivacyGuard::init(
            config.privacy.local_only,
            config.privacy.allowed_domains.clone(),
        );

        // Step 5: Initialize audio manager
        let audio_manager = Arc::new(CpalAudioManager::new()?);

        // Step 6: Initialize hardware detector
        let hardware_detector = Arc::new(CpuHardwareDetector::new());
        // Pre-detect hardware profile
        let _ = hardware_detector.detect();

        // Step 7: Initialize model manager
        let model_manager = Arc::new(LocalModelManager::new(config_store.data_dir())?);

        // Step 8: Initialize transcriber
        let threads = hardware_detector
            .profile()
            .map(|p| p.recommended_threads())
            .unwrap_or(1);
        let transcriber = Arc::new(WhisperCppTranscriber::new(threads));

        // Step 9: Initialize output manager
        let output_manager = Arc::new(ClipboardOutputManager::new(config.output.clone())?);

        info!(
            local_only = config.privacy.local_only,
            transcriber_threads = threads,
            "AppController initialized"
        );

        Ok(Self {
            config: RwLock::new(config),
            config_store,
            audio_manager,
            transcriber,
            model_manager,
            hardware_detector,
            output_manager,
            toggle_in_progress: AtomicBool::new(false),
            _log_guard: log_guard,
        })
    }

    /// Get the current configuration.
    pub fn config(&self) -> AppConfig {
        self.config.read().clone()
    }

    /// Update the configuration.
    pub fn update_config(&self, config: AppConfig) -> Result<(), DomainError> {
        // Update PrivacyGuard settings
        let guard = PrivacyGuard::global();
        guard.set_local_only(config.privacy.local_only);
        guard.set_allowed_domains(config.privacy.allowed_domains.clone());

        // Save to disk
        self.config_store.save(&config)?;

        // Update in-memory config
        *self.config.write() = config;

        info!("Configuration updated");
        Ok(())
    }

    /// Check if network is currently blocked.
    pub fn is_network_blocked(&self) -> bool {
        PrivacyGuard::global().is_network_blocked()
    }

    /// Get the data directory path.
    pub fn data_dir(&self) -> String {
        self.config_store.data_dir().to_string_lossy().to_string()
    }

    /// Get the logs directory path.
    pub fn logs_dir(&self) -> String {
        self.config_store.logs_dir().to_string_lossy().to_string()
    }

    /// Get the config file path.
    pub fn config_path(&self) -> String {
        self.config_store.config_path().to_string_lossy().to_string()
    }

    // ==================== Audio Methods ====================

    /// Start audio recording.
    pub async fn start_recording(&self) -> Result<(), DomainError> {
        self.audio_manager.start_recording().await
    }

    /// Stop audio recording and return the captured buffer.
    pub async fn stop_recording(&self) -> Result<AudioBuffer, DomainError> {
        self.audio_manager.stop_recording().await
    }

    /// Get current audio state.
    pub fn audio_state(&self) -> AudioState {
        self.audio_manager.state()
    }

    /// Get audio configuration.
    pub fn audio_config(&self) -> AudioConfig {
        self.audio_manager.config()
    }

    /// List available audio input devices.
    pub fn list_audio_devices(&self) -> Result<Vec<AudioDevice>, DomainError> {
        self.audio_manager.list_input_devices()
    }

    /// Select an audio input device.
    pub fn select_audio_device(&self, device_id: Option<&str>) -> Result<(), DomainError> {
        self.audio_manager.select_input_device(device_id)
    }

    /// Subscribe to audio events.
    pub fn subscribe_audio_events(&self) -> broadcast::Receiver<AudioEvent> {
        self.audio_manager.subscribe()
    }

    /// Attempt to recover from audio error state.
    pub async fn recover_audio(&self) -> Result<(), DomainError> {
        self.audio_manager.recover().await
    }

    /// Get current recording duration in seconds.
    pub fn recording_duration(&self) -> f32 {
        self.audio_manager.current_duration()
    }

    /// Get current audio input level (0.0-1.0).
    pub fn audio_level(&self) -> f32 {
        self.audio_manager.current_level()
    }

    /// Toggle recording: start if idle, stop + transcribe + inject if recording.
    ///
    /// This is the main entry point for the global shortcut flow.
    /// When recording is stopped, the audio is transcribed and the resulting
    /// text is injected into the active application via clipboard paste.
    ///
    /// Uses an atomic guard to prevent concurrent calls (e.g., from keyboard repeat).
    pub async fn toggle_recording(&self) -> Result<ToggleResult, DomainError> {
        // Guard against concurrent toggle calls (keyboard repeat, double-tap)
        if self
            .toggle_in_progress
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err(DomainError::Audio(
                "Toggle already in progress".to_string(),
            ));
        }

        // Ensure we reset the flag when we're done, even on error
        let result = self.toggle_recording_inner().await;
        self.toggle_in_progress.store(false, Ordering::SeqCst);
        result
    }

    /// Inner implementation of toggle_recording (without concurrency guard).
    async fn toggle_recording_inner(&self) -> Result<ToggleResult, DomainError> {
        match self.audio_state() {
            AudioState::Idle => {
                self.start_recording().await?;
                info!("Toggle: recording started");
                Ok(ToggleResult::Started)
            }
            AudioState::Recording => {
                // Stop recording
                let buffer = self.stop_recording().await?;
                info!(
                    duration_secs = buffer.duration_secs(),
                    samples = buffer.samples().len(),
                    "Toggle: recording stopped, starting transcription"
                );

                // Transcribe with VAD settings from config
                let config = {
                    let app_config = self.config.read();
                    TranscribeConfig {
                        language: if app_config.transcription.language == "auto" {
                            None
                        } else {
                            Some(app_config.transcription.language.clone())
                        },
                        vad_enabled: app_config.transcription.vad_enabled,
                        vad_no_speech_threshold: app_config.transcription.vad_no_speech_threshold,
                        vad_entropy_threshold: app_config.transcription.vad_entropy_threshold,
                        threads: 0, // Use default
                    }
                };

                let result = self.transcriber.transcribe(&buffer, &config).await?;
                // buffer is dropped here and zeroized automatically

                info!(
                    text_len = result.text.len(),
                    duration_ms = result.duration_ms,
                    "Toggle: transcription complete"
                );

                // Inject text into active application (skip if empty)
                let text = if result.text.is_empty() {
                    None
                } else {
                    self.output_manager.inject_text(&result.text).await?;
                    Some(result.text)
                };

                Ok(ToggleResult::Completed { text })
            }
            AudioState::DeviceLost | AudioState::Recovering => {
                Err(DomainError::Audio(
                    "Audio device unavailable, please wait for recovery".to_string(),
                ))
            }
            AudioState::Error => {
                Err(DomainError::Audio(
                    "Audio is in error state, please recover first".to_string(),
                ))
            }
        }
    }

    // ==================== Transcription Methods ====================

    /// Transcribe an audio buffer to text.
    pub async fn transcribe(
        &self,
        audio: AudioBuffer,
        config: Option<TranscribeConfig>,
    ) -> Result<TranscriptionResult, DomainError> {
        let config = config.unwrap_or_default();
        self.transcriber.transcribe(&audio, &config).await
    }

    /// Load a transcription model from the specified path.
    pub async fn load_model(&self, path: PathBuf) -> Result<(), DomainError> {
        self.transcriber.load_model(&path).await
    }

    /// Check if a transcription model is loaded.
    pub fn is_model_loaded(&self) -> bool {
        self.transcriber.is_model_loaded()
    }

    /// Unload the current transcription model.
    pub fn unload_model(&self) {
        self.transcriber.unload_model();
    }

    // ==================== Model Management Methods ====================

    /// Get the model catalog.
    pub fn model_catalog(&self) -> ModelCatalog {
        self.model_manager.catalog().clone()
    }

    /// List installed models.
    pub fn list_installed_models(&self) -> Result<Vec<InstalledModel>, DomainError> {
        self.model_manager.list_installed()
    }

    /// Check if a model is installed.
    pub fn is_model_installed(&self, model_id: &str, quant: Quantization) -> bool {
        self.model_manager.is_installed(model_id, quant)
    }

    /// Get the path to an installed model.
    pub fn model_path(&self, model_id: &str, quant: Quantization) -> Option<PathBuf> {
        self.model_manager.model_path(model_id, quant)
    }

    /// Download a model.
    pub async fn download_model(
        &self,
        model_id: &str,
        quant: Quantization,
        progress: Option<Box<dyn Fn(DownloadProgress) + Send + Sync>>,
    ) -> Result<InstalledModel, DomainError> {
        self.model_manager.download(model_id, quant, progress).await
    }

    /// Verify a model's integrity.
    pub fn verify_model(&self, model_id: &str, quant: Quantization) -> Result<bool, DomainError> {
        self.model_manager.verify(model_id, quant)
    }

    /// Delete an installed model.
    pub fn delete_model(&self, model_id: &str, quant: Quantization) -> Result<(), DomainError> {
        self.model_manager.delete(model_id, quant)
    }

    /// Get the models directory path.
    pub fn models_dir(&self) -> PathBuf {
        self.model_manager.models_dir()
    }

    // ==================== Hardware Methods ====================

    /// Get the hardware profile.
    pub fn hardware_profile(&self) -> Result<HardwareProfile, DomainError> {
        self.hardware_detector.detect()
    }

    /// Get the recommended model for this hardware.
    pub fn recommended_model(&self) -> Result<ModelRecommendation, DomainError> {
        self.hardware_detector
            .recommend_model(self.model_manager.catalog())
    }
}
