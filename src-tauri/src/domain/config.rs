use serde::{Deserialize, Serialize};

/// Privacy-related configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PrivacyConfig {
    /// When true, all network requests are blocked (default: true).
    pub local_only: bool,
    /// Allowed domains when local_only is false.
    pub allowed_domains: Vec<String>,
}

impl Default for PrivacyConfig {
    fn default() -> Self {
        Self {
            local_only: true,
            allowed_domains: Self::default_allowed_domains(),
        }
    }
}

impl PrivacyConfig {
    /// Default allowed domains for API and model downloads.
    pub fn default_allowed_domains() -> Vec<String> {
        vec![
            "api.openai.com".to_string(),
            "api.deepgram.com".to_string(),
            "huggingface.co".to_string(),
            "cdn-lfs.huggingface.co".to_string(),
            "cdn-lfs-us-1.huggingface.co".to_string(),
        ]
    }
}

/// Logging configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LoggingConfig {
    /// Log level: "trace", "debug", "info", "warn", "error".
    pub level: String,
    /// Enable file logging with rotation.
    pub file_logging: bool,
    /// Maximum number of log files to keep.
    pub max_files: u32,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            file_logging: true,
            max_files: 7,
        }
    }
}

/// UI configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiConfig {
    /// Show tray icon.
    pub show_tray: bool,
    /// Start minimized.
    pub start_minimized: bool,
    /// Theme: "system", "light", "dark".
    pub theme: String,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            show_tray: true,
            start_minimized: false,
            theme: "system".to_string(),
        }
    }
}

/// Transcription configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TranscriptionConfig {
    /// Selected model name.
    pub model: String,
    /// Language code (e.g., "en", "fr", "auto").
    pub language: String,
    /// Enable Voice Activity Detection.
    pub vad_enabled: bool,
    /// VAD: No-speech probability threshold (0.0-1.0).
    /// Higher values = more aggressive silence detection.
    /// Default 0.6 from whisper.cpp recommendations.
    pub vad_no_speech_threshold: f32,
    /// VAD: Entropy threshold for detecting non-speech.
    /// Default 2.4 from whisper.cpp recommendations.
    pub vad_entropy_threshold: f32,
}

impl Default for TranscriptionConfig {
    fn default() -> Self {
        Self {
            model: "whisper-small".to_string(),
            language: "auto".to_string(),
            vad_enabled: true,
            // Defaults from whisper.cpp:
            // https://github.com/ggerganov/whisper.cpp/blob/master/whisper.h
            vad_no_speech_threshold: 0.6,
            vad_entropy_threshold: 2.4,
        }
    }
}

/// Shortcut configuration.
///
/// NOTE: Currently only "Alt+Space" is supported as the shortcut.
/// Custom shortcut parsing is planned for a future release.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ShortcutConfig {
    /// Keyboard shortcut to toggle recording.
    /// Currently only "Alt+Space" is supported (other values are ignored).
    pub toggle_shortcut: String,
}

impl Default for ShortcutConfig {
    fn default() -> Self {
        Self {
            toggle_shortcut: "Alt+Space".to_string(),
        }
    }
}

/// Output/text injection configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct OutputConfig {
    /// Delay in ms before simulating paste (for clipboard sync).
    pub paste_delay_ms: u64,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            paste_delay_ms: 100,
        }
    }
}

/// Main application configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AppConfig {
    pub privacy: PrivacyConfig,
    pub logging: LoggingConfig,
    pub ui: UiConfig,
    pub transcription: TranscriptionConfig,
    pub shortcut: ShortcutConfig,
    pub output: OutputConfig,
}

impl AppConfig {
    /// Create a new AppConfig with default values.
    pub fn new() -> Self {
        Self::default()
    }
}
