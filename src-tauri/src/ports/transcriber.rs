use std::path::Path;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::domain::{AudioBuffer, DomainError};

/// Configuration for transcription.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscribeConfig {
    /// Target language (ISO 639-1 code, e.g., "en", "fr").
    /// None for auto-detection.
    pub language: Option<String>,
    /// Enable voice activity detection to skip silence.
    /// TODO(Phase 4): Integrate Silero VAD - currently unused.
    pub vad_enabled: bool,
    /// Number of threads to use (0 = auto).
    pub threads: u32,
}

impl Default for TranscribeConfig {
    fn default() -> Self {
        Self {
            language: None,
            vad_enabled: true,
            threads: 0,
        }
    }
}

/// Result of a transcription.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionResult {
    /// Transcribed text.
    pub text: String,
    /// Detected language (ISO 639-1 code).
    pub detected_language: Option<String>,
    /// Transcription duration in milliseconds.
    pub duration_ms: u64,
}

/// Capabilities of a transcription backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendCapabilities {
    /// Supported languages (ISO 639-1 codes).
    pub languages: Vec<String>,
    /// Whether streaming transcription is supported.
    pub streaming: bool,
    /// Whether the backend requires network access.
    pub requires_network: bool,
    /// Backend name for display.
    pub name: String,
}

/// Port for transcription operations.
///
/// Implementations handle the actual transcription using different backends
/// (local whisper.cpp, OpenAI API, etc.).
#[async_trait]
pub trait Transcriber: Send + Sync {
    /// Transcribe audio to text.
    ///
    /// The audio buffer is consumed after transcription (zeroed for privacy).
    async fn transcribe(
        &self,
        audio: &AudioBuffer,
        config: &TranscribeConfig,
    ) -> Result<TranscriptionResult, DomainError>;

    /// Get the capabilities of this transcription backend.
    fn capabilities(&self) -> BackendCapabilities;

    /// Check if the backend is currently available.
    ///
    /// For local backends, this checks if a model is loaded.
    /// For network backends, this checks connectivity.
    fn is_available(&self) -> bool;

    /// Load a model from the specified path.
    ///
    /// For network backends, this may be a no-op.
    async fn load_model(&self, path: &Path) -> Result<(), DomainError>;

    /// Unload the current model to free resources.
    fn unload_model(&self);

    /// Check if a model is currently loaded.
    fn is_model_loaded(&self) -> bool;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transcribe_config_default() {
        let config = TranscribeConfig::default();
        assert!(config.language.is_none());
        assert!(config.vad_enabled);
        assert_eq!(config.threads, 0);
    }
}
