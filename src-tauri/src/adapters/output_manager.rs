use std::time::Duration;

use arboard::Clipboard;
use async_trait::async_trait;
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use parking_lot::Mutex;
use tracing::{debug, info};

use crate::domain::config::OutputConfig;
use crate::domain::error::DomainError;
use crate::ports::OutputManager;

/// macOS implementation of OutputManager using clipboard + simulated paste.
///
/// Note: This replaces the user's clipboard content with the transcribed text.
/// The original clipboard content is NOT restored to avoid race conditions
/// where the user might paste before restoration completes.
pub struct ClipboardOutputManager {
    config: OutputConfig,
    clipboard: Mutex<Clipboard>,
}

impl ClipboardOutputManager {
    /// Create a new ClipboardOutputManager.
    pub fn new(config: OutputConfig) -> Result<Self, DomainError> {
        let clipboard = Clipboard::new()
            .map_err(|e| DomainError::Clipboard(format!("Failed to initialize clipboard: {}", e)))?;

        Ok(Self {
            config,
            clipboard: Mutex::new(clipboard),
        })
    }

    /// Set text to clipboard.
    fn set_clipboard_text(&self, text: &str) -> Result<(), DomainError> {
        let mut clipboard = self.clipboard.lock();
        clipboard
            .set_text(text)
            .map_err(|e| DomainError::Clipboard(format!("Failed to set clipboard text: {}", e)))?;
        debug!("Set clipboard text ({} chars)", text.len());
        Ok(())
    }

    /// Simulate Cmd+V paste on macOS.
    fn simulate_paste(&self) -> Result<(), DomainError> {
        let mut enigo = Enigo::new(&Settings::default())
            .map_err(|e| DomainError::InputSimulation(format!("Failed to create Enigo: {}", e)))?;

        // On macOS, use Meta (Command) key for paste
        enigo
            .key(Key::Meta, Direction::Press)
            .map_err(|e| DomainError::InputSimulation(format!("Failed to press Meta: {}", e)))?;

        enigo
            .key(Key::Unicode('v'), Direction::Click)
            .map_err(|e| DomainError::InputSimulation(format!("Failed to press V: {}", e)))?;

        enigo
            .key(Key::Meta, Direction::Release)
            .map_err(|e| DomainError::InputSimulation(format!("Failed to release Meta: {}", e)))?;

        debug!("Simulated Cmd+V paste");
        Ok(())
    }
}

#[async_trait]
impl OutputManager for ClipboardOutputManager {
    async fn inject_text(&self, text: &str) -> Result<(), DomainError> {
        if text.is_empty() {
            debug!("Empty text, skipping injection");
            return Ok(());
        }

        info!("Injecting transcribed text ({} chars)", text.len());

        // Step 1: Write transcribed text to clipboard
        self.set_clipboard_text(text)?;

        // Step 2: Wait for clipboard to sync
        let delay = Duration::from_millis(self.config.paste_delay_ms);
        tokio::time::sleep(delay).await;

        // Step 3: Simulate paste (Cmd+V on macOS)
        self.simulate_paste()?;

        info!("Text injection completed successfully");
        Ok(())
    }
}
