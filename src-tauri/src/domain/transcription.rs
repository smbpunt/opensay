use zeroize::Zeroize;

/// Audio buffer that is securely zeroed on drop.
/// Audio data never touches disk and is cleared from memory after transcription.
#[derive(Debug, Zeroize)]
#[zeroize(drop)]
pub struct AudioBuffer {
    /// PCM audio samples (16-bit mono, 16kHz).
    samples: Vec<i16>,
    /// Sample rate in Hz.
    sample_rate: u32,
    /// Number of channels (always 1 for our use case).
    channels: u8,
}

impl AudioBuffer {
    /// Create a new empty audio buffer.
    pub fn new(sample_rate: u32) -> Self {
        Self {
            samples: Vec::new(),
            sample_rate,
            channels: 1,
        }
    }

    /// Create an audio buffer with pre-allocated capacity.
    pub fn with_capacity(sample_rate: u32, capacity: usize) -> Self {
        Self {
            samples: Vec::with_capacity(capacity),
            sample_rate,
            channels: 1,
        }
    }

    /// Append samples to the buffer.
    pub fn push_samples(&mut self, samples: &[i16]) {
        self.samples.extend_from_slice(samples);
    }

    /// Get the samples as a slice.
    pub fn samples(&self) -> &[i16] {
        &self.samples
    }

    /// Get the sample rate.
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Get the number of channels.
    pub fn channels(&self) -> u8 {
        self.channels
    }

    /// Get the duration in seconds.
    pub fn duration_secs(&self) -> f32 {
        self.samples.len() as f32 / self.sample_rate as f32
    }

    /// Check if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// Get the number of samples.
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    /// Clear the buffer (samples are zeroed due to Zeroize).
    pub fn clear(&mut self) {
        self.samples.zeroize();
        self.samples.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_buffer_creation() {
        let buffer = AudioBuffer::new(16000);
        assert!(buffer.is_empty());
        assert_eq!(buffer.sample_rate(), 16000);
        assert_eq!(buffer.channels(), 1);
    }

    #[test]
    fn test_audio_buffer_push_samples() {
        let mut buffer = AudioBuffer::new(16000);
        buffer.push_samples(&[100, 200, 300]);
        assert_eq!(buffer.len(), 3);
        assert_eq!(buffer.samples(), &[100, 200, 300]);
    }

    #[test]
    fn test_audio_buffer_duration() {
        let mut buffer = AudioBuffer::new(16000);
        // 16000 samples = 1 second at 16kHz
        buffer.push_samples(&vec![0i16; 16000]);
        assert!((buffer.duration_secs() - 1.0).abs() < 0.001);
    }
}
