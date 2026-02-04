use async_trait::async_trait;

use crate::domain::error::DomainError;

/// Port for text output/injection.
///
/// Responsible for injecting transcribed text into the active application
/// via clipboard and simulated paste.
#[async_trait]
pub trait OutputManager: Send + Sync {
    /// Inject text into the active application.
    ///
    /// Implementation should:
    /// 1. Write the text to the clipboard
    /// 2. Wait for clipboard sync (platform-specific delay)
    /// 3. Simulate a paste command (Cmd+V on macOS, Ctrl+V on Windows/Linux)
    async fn inject_text(&self, text: &str) -> Result<(), DomainError>;
}
