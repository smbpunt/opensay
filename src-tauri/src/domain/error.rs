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
