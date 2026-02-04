use std::fs;
use std::path::PathBuf;

use tracing::{debug, info};

use crate::domain::{AppConfig, DomainError};
use crate::ports::ConfigStore;

/// TOML-based configuration store with OS-specific paths.
pub struct TomlConfigStore {
    data_dir: PathBuf,
}

impl TomlConfigStore {
    /// Create a new TomlConfigStore.
    /// Uses OS-specific application data directories.
    pub fn new() -> Result<Self, DomainError> {
        let data_dir = Self::get_data_dir()?;

        // Ensure the data directory exists
        fs::create_dir_all(&data_dir)?;

        info!(data_dir = ?data_dir, "ConfigStore initialized");

        Ok(Self { data_dir })
    }

    /// Get the OS-specific application data directory.
    /// - macOS: ~/Library/Application Support/OpenSay/
    /// - Windows: %APPDATA%\OpenSay\
    /// - Linux: ~/.config/OpenSay/
    fn get_data_dir() -> Result<PathBuf, DomainError> {
        #[cfg(target_os = "macos")]
        {
            dirs::data_dir()
                .map(|p| p.join("OpenSay"))
                .ok_or_else(|| DomainError::Config("Could not find application data directory".to_string()))
        }

        #[cfg(target_os = "windows")]
        {
            dirs::config_dir()
                .map(|p| p.join("OpenSay"))
                .ok_or_else(|| DomainError::Config("Could not find application data directory".to_string()))
        }

        #[cfg(target_os = "linux")]
        {
            dirs::config_dir()
                .map(|p| p.join("OpenSay"))
                .ok_or_else(|| DomainError::Config("Could not find application data directory".to_string()))
        }

        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            Err(DomainError::Config("Unsupported operating system".to_string()))
        }
    }

    /// Get the OS-specific log directory.
    /// - macOS: ~/Library/Application Support/OpenSay/logs/
    /// - Windows: %LOCALAPPDATA%\OpenSay\logs\
    /// - Linux: ~/.local/share/OpenSay/logs/
    fn get_logs_dir(&self) -> PathBuf {
        #[cfg(target_os = "macos")]
        {
            self.data_dir.join("logs")
        }

        #[cfg(target_os = "windows")]
        {
            dirs::data_local_dir()
                .map(|p| p.join("OpenSay").join("logs"))
                .unwrap_or_else(|| self.data_dir.join("logs"))
        }

        #[cfg(target_os = "linux")]
        {
            dirs::data_dir()
                .map(|p| p.join("OpenSay").join("logs"))
                .unwrap_or_else(|| self.data_dir.join("logs"))
        }

        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            self.data_dir.join("logs")
        }
    }
}

impl ConfigStore for TomlConfigStore {
    fn load(&self) -> Result<AppConfig, DomainError> {
        let config_path = self.config_path();

        if config_path.exists() {
            debug!(path = ?config_path, "Loading configuration");
            let content = fs::read_to_string(&config_path)?;
            let config: AppConfig = toml::from_str(&content)?;
            info!(path = ?config_path, "Configuration loaded");
            Ok(config)
        } else {
            info!(path = ?config_path, "Configuration file not found, creating default");
            let config = AppConfig::new();
            self.save(&config)?;
            Ok(config)
        }
    }

    fn save(&self, config: &AppConfig) -> Result<(), DomainError> {
        let config_path = self.config_path();

        // Ensure parent directory exists
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(config)?;
        fs::write(&config_path, content)?;

        info!(path = ?config_path, "Configuration saved");
        Ok(())
    }

    fn config_path(&self) -> PathBuf {
        self.data_dir.join("config.toml")
    }

    fn data_dir(&self) -> PathBuf {
        self.data_dir.clone()
    }

    fn logs_dir(&self) -> PathBuf {
        self.get_logs_dir()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_config_store_paths() {
        let store = TomlConfigStore::new().unwrap();

        let config_path = store.config_path();
        assert!(config_path.ends_with("config.toml"));

        let logs_dir = store.logs_dir();
        assert!(logs_dir.to_string_lossy().contains("logs"));
    }

    #[test]
    fn test_config_roundtrip() {
        // Use a temporary directory for testing
        let temp_dir = env::temp_dir().join("opensay_test");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let store = TomlConfigStore {
            data_dir: temp_dir.clone(),
        };

        // Create and save a config
        let mut config = AppConfig::new();
        config.privacy.local_only = false;
        config.logging.level = "debug".to_string();

        store.save(&config).unwrap();

        // Load it back
        let loaded = store.load().unwrap();
        assert!(!loaded.privacy.local_only);
        assert_eq!(loaded.logging.level, "debug");

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
