pub mod audio;
pub mod config;
pub mod error;
pub mod hardware;
pub mod model;
pub mod transcription;

pub use audio::{AtomicAudioState, AudioConfig, AudioDevice, AudioEvent, AudioState};
pub use config::AppConfig;
pub use error::DomainError;
pub use hardware::{CpuArch, HardwareProfile, ModelRecommendation, OsType, SimdCapabilities};
pub use model::{DownloadProgress, InstalledModel, ModelCatalog, Quantization};
pub use transcription::AudioBuffer;
