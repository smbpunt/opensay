use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU8, Ordering};

/// Audio capture state machine.
///
/// State transitions:
/// - Idle -> Recording (start_recording)
/// - Recording -> Idle (stop_recording, returns AudioBuffer)
/// - Recording -> DeviceLost (device disconnected, automatic)
/// - DeviceLost -> Recovering -> Idle (recover, user-initiated)
/// - Recovering -> Error (after max_recovery_attempts failures)
/// - Error -> Recovering -> Idle (recover, user-initiated)
///
/// Note: Recovery always transitions to Idle, not back to Recording.
/// This is intentional - the user must explicitly restart recording
/// after a device loss to avoid unexpected audio capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum AudioState {
    /// Ready to record, no active capture.
    Idle = 0,
    /// Actively capturing audio.
    Recording = 1,
    /// Input device was disconnected.
    DeviceLost = 2,
    /// Attempting to recover from device loss.
    Recovering = 3,
    /// Unrecoverable error occurred.
    Error = 4,
}

impl AudioState {
    /// Check if recording can be started from this state.
    #[must_use]
    pub fn can_start_recording(&self) -> bool {
        matches!(self, AudioState::Idle)
    }

    /// Check if recording can be stopped from this state.
    #[must_use]
    pub fn can_stop_recording(&self) -> bool {
        matches!(self, AudioState::Recording)
    }

    /// Check if recovery can be attempted from this state.
    /// Recovery is allowed from DeviceLost and Error states.
    #[must_use]
    pub fn can_recover(&self) -> bool {
        matches!(self, AudioState::DeviceLost | AudioState::Error)
    }
}

impl From<u8> for AudioState {
    fn from(value: u8) -> Self {
        match value {
            0 => AudioState::Idle,
            1 => AudioState::Recording,
            2 => AudioState::DeviceLost,
            3 => AudioState::Recovering,
            4 => AudioState::Error,
            _ => AudioState::Error, // Unknown states map to Error
        }
    }
}

impl From<AudioState> for u8 {
    fn from(state: AudioState) -> Self {
        state as u8
    }
}

/// Atomic wrapper for AudioState for lock-free reads.
#[derive(Debug)]
pub struct AtomicAudioState(AtomicU8);

impl AtomicAudioState {
    pub fn new(state: AudioState) -> Self {
        Self(AtomicU8::new(state.into()))
    }

    pub fn load(&self) -> AudioState {
        self.0.load(Ordering::Acquire).into()
    }

    pub fn store(&self, state: AudioState) {
        self.0.store(state.into(), Ordering::Release);
    }

    /// Compare and swap, returns true if successful.
    pub fn compare_exchange(&self, current: AudioState, new: AudioState) -> bool {
        self.0
            .compare_exchange(current.into(), new.into(), Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }
}

impl Default for AtomicAudioState {
    fn default() -> Self {
        Self::new(AudioState::Idle)
    }
}

/// Audio capture configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    /// Maximum recording duration in seconds (ring buffer size).
    pub buffer_duration_secs: u32,
    /// Target sample rate in Hz.
    pub sample_rate: u32,
    /// Maximum recovery attempts before transitioning to Error state.
    pub max_recovery_attempts: u32,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            buffer_duration_secs: 60, // 60 second ring buffer
            sample_rate: 16_000,      // 16kHz for Whisper
            max_recovery_attempts: 3,
        }
    }
}

impl AudioConfig {
    /// Calculate the ring buffer capacity in samples.
    pub fn buffer_capacity(&self) -> usize {
        self.buffer_duration_secs as usize * self.sample_rate as usize
    }
}

/// Events emitted by the audio capture system.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "data")]
pub enum AudioEvent {
    /// Audio state changed.
    StateChanged {
        from: AudioState,
        to: AudioState,
    },
    /// Input device was lost.
    DeviceLost {
        device_name: String,
    },
    /// Successfully recovered from device loss.
    RecoverySuccess {
        device_name: String,
    },
    /// Failed to recover after max attempts.
    RecoveryFailed {
        attempts: u32,
        last_error: String,
    },
    /// An error occurred.
    Error {
        message: String,
    },
    /// Audio level update (for visualization).
    LevelUpdate {
        /// RMS level normalized to 0.0-1.0.
        level: f32,
    },
}

/// Input audio device information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDevice {
    /// Unique device identifier.
    pub id: String,
    /// Human-readable device name.
    pub name: String,
    /// Whether this is the system default device.
    pub is_default: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_state_can_start_recording() {
        assert!(AudioState::Idle.can_start_recording());
        assert!(!AudioState::Recording.can_start_recording());
        assert!(!AudioState::DeviceLost.can_start_recording());
        assert!(!AudioState::Recovering.can_start_recording());
        assert!(!AudioState::Error.can_start_recording());
    }

    #[test]
    fn test_audio_state_can_stop_recording() {
        assert!(!AudioState::Idle.can_stop_recording());
        assert!(AudioState::Recording.can_stop_recording());
        assert!(!AudioState::DeviceLost.can_stop_recording());
        assert!(!AudioState::Recovering.can_stop_recording());
        assert!(!AudioState::Error.can_stop_recording());
    }

    #[test]
    fn test_audio_state_can_recover() {
        assert!(!AudioState::Idle.can_recover());
        assert!(!AudioState::Recording.can_recover());
        assert!(AudioState::DeviceLost.can_recover()); // Can recover from device loss
        assert!(!AudioState::Recovering.can_recover()); // Already recovering
        assert!(AudioState::Error.can_recover()); // Can recover from error
    }

    #[test]
    fn test_audio_state_roundtrip() {
        for state in [
            AudioState::Idle,
            AudioState::Recording,
            AudioState::DeviceLost,
            AudioState::Recovering,
            AudioState::Error,
        ] {
            let value: u8 = state.into();
            let recovered: AudioState = value.into();
            assert_eq!(state, recovered);
        }
    }

    #[test]
    fn test_atomic_audio_state() {
        let atomic = AtomicAudioState::new(AudioState::Idle);
        assert_eq!(atomic.load(), AudioState::Idle);

        atomic.store(AudioState::Recording);
        assert_eq!(atomic.load(), AudioState::Recording);

        // Successful CAS
        assert!(atomic.compare_exchange(AudioState::Recording, AudioState::DeviceLost));
        assert_eq!(atomic.load(), AudioState::DeviceLost);

        // Failed CAS (wrong current value)
        assert!(!atomic.compare_exchange(AudioState::Idle, AudioState::Recording));
        assert_eq!(atomic.load(), AudioState::DeviceLost); // Unchanged
    }

    #[test]
    fn test_audio_config_default() {
        let config = AudioConfig::default();
        assert_eq!(config.buffer_duration_secs, 60);
        assert_eq!(config.sample_rate, 16_000);
        assert_eq!(config.max_recovery_attempts, 3);
    }

    #[test]
    fn test_audio_config_buffer_capacity() {
        let config = AudioConfig::default();
        // 60 seconds * 16000 samples/sec = 960000 samples
        assert_eq!(config.buffer_capacity(), 960_000);
    }
}
