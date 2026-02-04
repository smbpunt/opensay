use std::sync::Arc;

use parking_lot::RwLock;
use tokio::sync::broadcast;
use tracing::info;
use tracing_appender::non_blocking::WorkerGuard;

use crate::adapters::{CpalAudioManager, PrivacyGuard, TomlConfigStore};
use crate::domain::{AppConfig, AudioBuffer, AudioConfig, AudioDevice, AudioEvent, AudioState, DomainError};
use crate::infrastructure::init_logging;
use crate::ports::{AudioManager, ConfigStore, HttpClient};

/// Application controller that orchestrates initialization and manages global state.
pub struct AppController {
    config: RwLock<AppConfig>,
    config_store: Arc<TomlConfigStore>,
    audio_manager: Arc<CpalAudioManager>,
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

        info!(
            local_only = config.privacy.local_only,
            "AppController initialized"
        );

        Ok(Self {
            config: RwLock::new(config),
            config_store,
            audio_manager,
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
}
