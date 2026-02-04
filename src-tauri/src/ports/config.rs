use std::path::PathBuf;

use crate::domain::{AppConfig, DomainError};

/// Configuration store port for persisting and loading app configuration.
pub trait ConfigStore: Send + Sync {
    /// Load configuration from persistent storage.
    /// Creates default config if none exists.
    fn load(&self) -> Result<AppConfig, DomainError>;

    /// Save configuration to persistent storage.
    fn save(&self, config: &AppConfig) -> Result<(), DomainError>;

    /// Get the path to the configuration file.
    fn config_path(&self) -> PathBuf;

    /// Get the path to the application data directory.
    fn data_dir(&self) -> PathBuf;

    /// Get the path to the logs directory.
    fn logs_dir(&self) -> PathBuf;
}
