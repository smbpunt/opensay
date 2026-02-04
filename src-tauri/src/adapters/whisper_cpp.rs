use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use parking_lot::RwLock;
use tracing::{debug, info};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use crate::domain::{AudioBuffer, DomainError};
use crate::ports::{BackendCapabilities, TranscribeConfig, Transcriber, TranscriptionResult};

/// Transcriber implementation using whisper.cpp via whisper-rs.
pub struct WhisperCppTranscriber {
    context: RwLock<Option<Arc<WhisperContext>>>,
    threads: u32,
}

impl WhisperCppTranscriber {
    /// Create a new WhisperCppTranscriber.
    ///
    /// The `threads` parameter specifies the number of threads to use.
    /// 0 means auto-detect (cores - 1).
    pub fn new(threads: u32) -> Self {
        let actual_threads = if threads == 0 {
            std::thread::available_parallelism()
                .map(|p| std::cmp::max(1, p.get() as u32 - 1))
                .unwrap_or(1)
        } else {
            threads
        };

        info!(threads = actual_threads, "WhisperCppTranscriber created");

        Self {
            context: RwLock::new(None),
            threads: actual_threads,
        }
    }

    /// Convert i16 samples to f32 (whisper expects f32 samples in range [-1, 1]).
    fn convert_samples(samples: &[i16]) -> Vec<f32> {
        samples.iter().map(|&s| s as f32 / 32768.0).collect()
    }
}

#[async_trait]
impl Transcriber for WhisperCppTranscriber {
    async fn transcribe(
        &self,
        audio: &AudioBuffer,
        config: &TranscribeConfig,
    ) -> Result<TranscriptionResult, DomainError> {
        let context = self.context.read().clone();
        let ctx = context.ok_or_else(|| DomainError::Whisper("No model loaded".to_string()))?;

        // Validate sample rate
        if audio.sample_rate() != 16000 {
            return Err(DomainError::Whisper(format!(
                "Expected 16kHz audio, got {}Hz",
                audio.sample_rate()
            )));
        }

        if audio.is_empty() {
            return Ok(TranscriptionResult {
                text: String::new(),
                detected_language: None,
                duration_ms: 0,
            });
        }

        // Convert samples
        let samples = Self::convert_samples(audio.samples());
        // Allow per-call thread override for batch processing scenarios
        // where different transcriptions may need different resource allocation.
        // Default (0) uses the auto-detected optimal thread count.
        let threads = if config.threads > 0 {
            config.threads
        } else {
            self.threads
        };

        debug!(
            samples = samples.len(),
            duration_secs = audio.duration_secs(),
            threads = threads,
            "Starting transcription"
        );

        let start = std::time::Instant::now();

        // Run transcription in blocking task (CPU-bound)
        let language = config.language.clone();
        let vad_enabled = config.vad_enabled;
        let vad_no_speech = config.vad_no_speech_threshold;
        let vad_entropy = config.vad_entropy_threshold;
        let result = tokio::task::spawn_blocking(move || {
            let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });

            params.set_n_threads(threads as i32);
            params.set_print_progress(false);
            params.set_print_realtime(false);
            params.set_print_timestamps(false);

            // Set language if specified, otherwise auto-detect
            if let Some(ref lang) = language {
                params.set_language(Some(lang));
            }

            // VAD parameters - filter silence and non-speech tokens
            if vad_enabled {
                params.set_no_speech_thold(vad_no_speech);
                params.set_entropy_thold(vad_entropy);
                params.set_suppress_non_speech_tokens(true);
            }

            // Create state for this transcription
            let mut state = ctx.create_state().map_err(|e| {
                DomainError::Whisper(format!("Failed to create whisper state: {}", e))
            })?;

            // Run inference
            state.full(params, &samples).map_err(|e| {
                DomainError::Whisper(format!("Transcription failed: {}", e))
            })?;

            // Collect results
            let num_segments = state.full_n_segments().map_err(|e| {
                DomainError::Whisper(format!("Failed to get segment count: {}", e))
            })?;

            let mut text = String::new();
            for i in 0..num_segments {
                if let Ok(segment_text) = state.full_get_segment_text(i) {
                    text.push_str(&segment_text);
                }
            }

            // Get detected language (if available)
            let detected_language = state
                .full_lang_id_from_state()
                .ok()
                .and_then(|id| whisper_rs::get_lang_str(id).map(|s| s.to_string()));

            Ok::<(String, Option<String>), DomainError>((text.trim().to_string(), detected_language))
        })
        .await
        .map_err(|e| DomainError::Whisper(format!("Task join error: {}", e)))??;

        let duration_ms = start.elapsed().as_millis() as u64;

        info!(
            text_len = result.0.len(),
            duration_ms = duration_ms,
            detected_language = ?result.1,
            "Transcription complete"
        );

        Ok(TranscriptionResult {
            text: result.0,
            detected_language: result.1,
            duration_ms,
        })
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            languages: vec![
                "en".to_string(),
                "fr".to_string(),
                "de".to_string(),
                "es".to_string(),
                "it".to_string(),
                "pt".to_string(),
                "nl".to_string(),
                "pl".to_string(),
                "ru".to_string(),
                "ja".to_string(),
                "zh".to_string(),
                "ko".to_string(),
            ],
            streaming: false,
            requires_network: false,
            name: "whisper.cpp".to_string(),
        }
    }

    fn is_available(&self) -> bool {
        self.context.read().is_some()
    }

    async fn load_model(&self, path: &Path) -> Result<(), DomainError> {
        if !path.exists() {
            return Err(DomainError::ModelNotFound(
                path.to_string_lossy().to_string(),
            ));
        }

        info!(path = ?path, "Loading whisper model");

        let path_str = path.to_string_lossy().to_string();

        // Load model in blocking task (I/O bound)
        let ctx = tokio::task::spawn_blocking(move || {
            WhisperContext::new_with_params(&path_str, WhisperContextParameters::default())
                .map_err(|e| DomainError::Whisper(format!("Failed to load model: {}", e)))
        })
        .await
        .map_err(|e| DomainError::Whisper(format!("Task join error: {}", e)))??;

        *self.context.write() = Some(Arc::new(ctx));

        info!(path = ?path, "Whisper model loaded successfully");
        Ok(())
    }

    fn unload_model(&self) {
        let had_model = self.context.read().is_some();
        *self.context.write() = None;

        if had_model {
            info!("Whisper model unloaded");
        }
    }

    fn is_model_loaded(&self) -> bool {
        self.context.read().is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sample_conversion() {
        let samples = vec![0i16, 16384, -16384, 32767, -32768];
        let converted = WhisperCppTranscriber::convert_samples(&samples);

        assert!((converted[0] - 0.0).abs() < 0.001);
        assert!((converted[1] - 0.5).abs() < 0.001);
        assert!((converted[2] - -0.5).abs() < 0.001);
        assert!((converted[3] - 1.0).abs() < 0.001);
        assert!((converted[4] - -1.0).abs() < 0.001);
    }

    #[test]
    fn test_transcriber_creation() {
        let transcriber = WhisperCppTranscriber::new(4);
        assert!(!transcriber.is_available());
        assert!(!transcriber.is_model_loaded());
    }

    #[test]
    fn test_capabilities() {
        let transcriber = WhisperCppTranscriber::new(4);
        let caps = transcriber.capabilities();

        assert_eq!(caps.name, "whisper.cpp");
        assert!(!caps.requires_network);
        assert!(!caps.streaming);
        assert!(caps.languages.contains(&"en".to_string()));
    }
}
