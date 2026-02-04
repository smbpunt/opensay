pub mod audio;
pub mod config;
pub mod hardware;
pub mod http;
pub mod model_manager;
pub mod transcriber;

pub use audio::AudioManager;
pub use config::ConfigStore;
pub use hardware::HardwareDetector;
pub use http::HttpClient;
pub use model_manager::ModelManager;
pub use transcriber::{BackendCapabilities, TranscribeConfig, Transcriber, TranscriptionResult};
