use crate::domain::audio::AudioState;
use thiserror::Error;

/// Domain-level errors for OpenSay.
#[derive(Error, Debug)]
pub enum DomainError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Network request blocked: {reason}")]
    NetworkBlocked { reason: String },

    #[error("HTTP request failed: {0}")]
    HttpRequest(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("IO error: {0}")]
    Io(String),

    #[error("Audio error: {0}")]
    Audio(String),

    #[error("Transcription error: {0}")]
    Transcription(String),

    #[error("Audio device error: {message}")]
    AudioDevice { message: String },

    #[error("Invalid audio state transition from {from:?} to {to:?}")]
    AudioStateTransition { from: AudioState, to: AudioState },

    #[error("Not currently recording")]
    AudioNotRecording,

    #[error("Already recording")]
    AudioAlreadyRecording,

    #[error("Model error: {0}")]
    Model(String),

    #[error("Model not found: {0}")]
    ModelNotFound(String),

    #[error("Model verification failed: expected {expected}, got {actual}")]
    ModelVerification { expected: String, actual: String },

    #[error("Model download failed: {0}")]
    ModelDownload(String),

    #[error("Hardware detection error: {0}")]
    Hardware(String),

    #[error("Whisper error: {0}")]
    Whisper(String),
}

impl From<std::io::Error> for DomainError {
    fn from(err: std::io::Error) -> Self {
        DomainError::Io(err.to_string())
    }
}

impl From<toml::de::Error> for DomainError {
    fn from(err: toml::de::Error) -> Self {
        DomainError::Config(err.to_string())
    }
}

impl From<toml::ser::Error> for DomainError {
    fn from(err: toml::ser::Error) -> Self {
        DomainError::Serialization(err.to_string())
    }
}

impl From<serde_json::Error> for DomainError {
    fn from(err: serde_json::Error) -> Self {
        DomainError::Serialization(err.to_string())
    }
}
