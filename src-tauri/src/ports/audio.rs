use async_trait::async_trait;
use tokio::sync::broadcast;

use crate::domain::{AudioBuffer, AudioConfig, AudioDevice, AudioEvent, AudioState, DomainError};

/// Port for audio capture operations.
///
/// Implementations handle platform-specific audio capture, device management,
/// and hot-plug recovery.
#[async_trait]
pub trait AudioManager: Send + Sync {
    /// Start recording audio from the selected input device.
    ///
    /// Returns an error if already recording or no device is available.
    async fn start_recording(&self) -> Result<(), DomainError>;

    /// Stop recording and return the captured audio buffer.
    ///
    /// The buffer contains PCM samples at 16kHz mono.
    /// Returns an error if not currently recording.
    async fn stop_recording(&self) -> Result<AudioBuffer, DomainError>;

    /// Get the current audio capture state.
    fn state(&self) -> AudioState;

    /// Get the audio configuration.
    fn config(&self) -> AudioConfig;

    /// List available audio input devices.
    fn list_input_devices(&self) -> Result<Vec<AudioDevice>, DomainError>;

    /// Select an input device by ID, or use the system default if None.
    fn select_input_device(&self, device_id: Option<&str>) -> Result<(), DomainError>;

    /// Subscribe to audio events.
    fn subscribe(&self) -> broadcast::Receiver<AudioEvent>;

    /// Attempt to recover from an error state.
    ///
    /// This is only valid when in the Error state.
    async fn recover(&self) -> Result<(), DomainError>;

    /// Get the current recording duration in seconds.
    ///
    /// Returns 0.0 if not recording.
    fn current_duration(&self) -> f32;

    /// Get the current audio input level (0.0 - 1.0).
    ///
    /// Returns 0.0 if not recording.
    fn current_level(&self) -> f32;
}
