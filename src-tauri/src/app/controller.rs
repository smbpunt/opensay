use std::sync::Arc;

use parking_lot::RwLock;
use tracing::info;
use tracing_appender::non_blocking::WorkerGuard;

use crate::adapters::{PrivacyGuard, TomlConfigStore};
use crate::domain::{AppConfig, DomainError};
use crate::infrastructure::init_logging;
use crate::ports::{ConfigStore, HttpClient};

/// Application controller that orchestrates initialization and manages global state.
pub struct AppController {
    config: RwLock<AppConfig>,
    config_store: Arc<TomlConfigStore>,
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

        info!(
            local_only = config.privacy.local_only,
            "AppController initialized"
        );

        Ok(Self {
            config: RwLock::new(config),
            config_store,
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
}
