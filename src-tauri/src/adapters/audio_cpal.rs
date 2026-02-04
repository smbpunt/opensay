use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Instant;

use async_trait::async_trait;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SampleFormat, Stream, StreamConfig};
use parking_lot::{Mutex, RwLock};
use ringbuf::traits::{Consumer, Observer, Producer, Split};
use ringbuf::HeapRb;
use tokio::sync::{broadcast, mpsc, oneshot};
use tracing::{debug, error, info, warn};

use crate::domain::{
    AtomicAudioState, AudioBuffer, AudioConfig, AudioDevice, AudioEvent, AudioState, DomainError,
};
use crate::ports::AudioManager;

/// Lock-free ring buffer for audio samples.
type RingProducer = ringbuf::HeapProd<i16>;
type RingConsumer = ringbuf::HeapCons<i16>;

/// Commands sent to the audio thread.
enum AudioCommand {
    Start {
        reply: oneshot::Sender<Result<(), DomainError>>,
    },
    Stop {
        reply: oneshot::Sender<Result<Vec<i16>, DomainError>>,
    },
    Shutdown,
}

/// Audio processing utilities.
mod audio_processing {
    use super::*;

    pub fn get_device(selected_device_id: Option<&str>) -> Result<Device, DomainError> {
        let host = cpal::default_host();

        if let Some(id) = selected_device_id {
            let devices = host.input_devices().map_err(|e| DomainError::AudioDevice {
                message: format!("Failed to enumerate devices: {}", e),
            })?;

            for device in devices {
                if let Ok(name) = device.name() {
                    if name == id {
                        return Ok(device);
                    }
                }
            }
            warn!(device_id = %id, "Selected device not found, falling back to default");
        }

        host.default_input_device()
            .ok_or_else(|| DomainError::AudioDevice {
                message: "No default input device available".to_string(),
            })
    }

    pub fn build_stream_config(device: &Device) -> Result<StreamConfig, DomainError> {
        let supported = device.default_input_config().map_err(|e| DomainError::AudioDevice {
            message: format!("Failed to get default config: {}", e),
        })?;

        debug!(
            sample_rate = ?supported.sample_rate(),
            channels = supported.channels(),
            format = ?supported.sample_format(),
            "Device default config"
        );

        Ok(StreamConfig {
            channels: supported.channels(),
            sample_rate: supported.sample_rate(),
            buffer_size: cpal::BufferSize::Default,
        })
    }

    pub fn build_stream(
        device: &Device,
        config: &StreamConfig,
        sample_format: SampleFormat,
        target_sample_rate: u32,
        mut producer: RingProducer,
        state: Arc<AtomicAudioState>,
        event_sender: broadcast::Sender<AudioEvent>,
        current_level: Arc<AtomicU32>,
    ) -> Result<Stream, DomainError> {
        let channels = config.channels as usize;
        let device_sample_rate = config.sample_rate.0;

        // Calculate samples_per_update based on TARGET rate since we count resampled samples
        let samples_per_update = (target_sample_rate / 10) as usize;
        let mut sample_counter = 0usize;
        let mut level_samples = Vec::with_capacity(samples_per_update);

        let state_err = Arc::clone(&state);
        let event_sender_err = event_sender.clone();

        let stream = match sample_format {
            SampleFormat::I16 => device.build_input_stream(
                config,
                move |data: &[i16], _: &cpal::InputCallbackInfo| {
                    process_samples_i16(
                        data,
                        channels,
                        device_sample_rate,
                        target_sample_rate,
                        &mut producer,
                        &mut level_samples,
                        &mut sample_counter,
                        samples_per_update,
                        &event_sender,
                        &current_level,
                    );
                },
                move |err| {
                    error!(?err, "Audio stream error");
                    handle_stream_error(&state_err, &event_sender_err);
                },
                None,
            ),
            SampleFormat::F32 => device.build_input_stream(
                config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    let i16_data: Vec<i16> = data
                        .iter()
                        .map(|&s| (s.clamp(-1.0, 1.0) * 32767.0) as i16)
                        .collect();

                    process_samples_i16(
                        &i16_data,
                        channels,
                        device_sample_rate,
                        target_sample_rate,
                        &mut producer,
                        &mut level_samples,
                        &mut sample_counter,
                        samples_per_update,
                        &event_sender,
                        &current_level,
                    );
                },
                move |err| {
                    error!(?err, "Audio stream error");
                    handle_stream_error(&state_err, &event_sender_err);
                },
                None,
            ),
            _ => {
                return Err(DomainError::AudioDevice {
                    message: format!("Unsupported sample format: {:?}", sample_format),
                });
            }
        }
        .map_err(|e| DomainError::AudioDevice {
            message: format!("Failed to build stream: {}", e),
        })?;

        Ok(stream)
    }

    #[allow(clippy::too_many_arguments)]
    fn process_samples_i16(
        data: &[i16],
        channels: usize,
        device_sample_rate: u32,
        target_sample_rate: u32,
        producer: &mut RingProducer,
        level_samples: &mut Vec<i16>,
        sample_counter: &mut usize,
        samples_per_update: usize,
        event_sender: &broadcast::Sender<AudioEvent>,
        current_level: &AtomicU32,
    ) {
        // Convert stereo to mono
        let mono_samples: Vec<i16> = if channels > 1 {
            data.chunks(channels)
                .map(|chunk| {
                    let sum: i32 = chunk.iter().map(|&s| s as i32).sum();
                    (sum / channels as i32) as i16
                })
                .collect()
        } else {
            data.to_vec()
        };

        // Resample if needed
        let resampled = if device_sample_rate != target_sample_rate {
            resample(&mono_samples, device_sample_rate, target_sample_rate)
        } else {
            mono_samples
        };

        // Write to ring buffer
        let _ = producer.push_slice(&resampled);

        // Update level periodically
        level_samples.extend_from_slice(&resampled);
        *sample_counter += resampled.len();

        if *sample_counter >= samples_per_update {
            let level = calculate_rms(level_samples);
            current_level.store(level.to_bits(), Ordering::Relaxed);
            let _ = event_sender.send(AudioEvent::LevelUpdate { level });
            level_samples.clear();
            *sample_counter = 0;
        }
    }

    pub fn calculate_rms(samples: &[i16]) -> f32 {
        if samples.is_empty() {
            return 0.0;
        }
        let sum_squares: f64 = samples.iter().map(|&s| (s as f64).powi(2)).sum();
        let rms = (sum_squares / samples.len() as f64).sqrt();
        (rms / 32767.0).min(1.0) as f32
    }

    pub fn resample(samples: &[i16], from_rate: u32, to_rate: u32) -> Vec<i16> {
        if from_rate == to_rate || samples.is_empty() {
            return samples.to_vec();
        }

        let ratio = from_rate as f64 / to_rate as f64;
        let output_len = (samples.len() as f64 / ratio).ceil() as usize;
        let mut output = Vec::with_capacity(output_len);

        for i in 0..output_len {
            let src_pos = i as f64 * ratio;
            let src_idx = src_pos.floor() as usize;
            let frac = src_pos.fract();

            let sample = if src_idx + 1 < samples.len() {
                let s0 = samples[src_idx] as f64;
                let s1 = samples[src_idx + 1] as f64;
                (s0 + (s1 - s0) * frac) as i16
            } else if src_idx < samples.len() {
                samples[src_idx]
            } else {
                0
            };
            output.push(sample);
        }
        output
    }

    fn handle_stream_error(state: &AtomicAudioState, event_sender: &broadcast::Sender<AudioEvent>) {
        let current = state.load();
        if current == AudioState::Recording {
            state.store(AudioState::DeviceLost);
            let _ = event_sender.send(AudioEvent::DeviceLost {
                device_name: "Unknown".to_string(),
            });
        }
    }
}

/// Audio thread runner - creates Stream on the audio thread.
fn audio_thread_main(
    config: AudioConfig,
    selected_device_id: Arc<RwLock<Option<String>>>,
    state: Arc<AtomicAudioState>,
    event_sender: broadcast::Sender<AudioEvent>,
    current_level: Arc<AtomicU32>,
    mut cmd_rx: mpsc::Receiver<AudioCommand>,
) {
    // Stream is kept here on the audio thread (not Send)
    let mut stream: Option<Stream> = None;
    let mut ring_consumer: Option<RingConsumer> = None;

    while let Some(cmd) = cmd_rx.blocking_recv() {
        match cmd {
            AudioCommand::Start { reply } => {
                let result = (|| -> Result<(), DomainError> {
                    if !state.load().can_start_recording() {
                        return Err(DomainError::AudioAlreadyRecording);
                    }

                    let device_id = selected_device_id.read().clone();
                    let device = audio_processing::get_device(device_id.as_deref())?;
                    let device_name = device.name().unwrap_or_else(|_| "Unknown".to_string());
                    let stream_config = audio_processing::build_stream_config(&device)?;

                    let capacity = config.buffer_capacity();
                    let ring = HeapRb::<i16>::new(capacity);
                    let (producer, consumer) = ring.split();

                    let sample_format = device.default_input_config().map_err(|e| DomainError::AudioDevice {
                        message: format!("Failed to get config: {}", e),
                    })?.sample_format();

                    let new_stream = audio_processing::build_stream(
                        &device,
                        &stream_config,
                        sample_format,
                        config.sample_rate,
                        producer,
                        Arc::clone(&state),
                        event_sender.clone(),
                        Arc::clone(&current_level),
                    )?;

                    new_stream.play().map_err(|e| DomainError::AudioDevice {
                        message: format!("Failed to start stream: {}", e),
                    })?;

                    stream = Some(new_stream);
                    ring_consumer = Some(consumer);

                    let from = state.load();
                    state.store(AudioState::Recording);
                    let _ = event_sender.send(AudioEvent::StateChanged {
                        from,
                        to: AudioState::Recording,
                    });

                    info!(device = %device_name, "Recording started");
                    Ok(())
                })();
                let _ = reply.send(result);
            }
            AudioCommand::Stop { reply } => {
                let result = (|| -> Result<Vec<i16>, DomainError> {
                    if !state.load().can_stop_recording() {
                        return Err(DomainError::AudioNotRecording);
                    }

                    // Stop and drop the stream
                    stream.take();

                    // Drain the ring buffer
                    let mut consumer = ring_consumer.take().ok_or(DomainError::AudioNotRecording)?;

                    let available = consumer.occupied_len();
                    let mut samples = vec![0i16; available];
                    let read = consumer.pop_slice(&mut samples);
                    samples.truncate(read);

                    // Reset level
                    current_level.store(0f32.to_bits(), Ordering::Relaxed);

                    let from = state.load();
                    state.store(AudioState::Idle);
                    let _ = event_sender.send(AudioEvent::StateChanged {
                        from,
                        to: AudioState::Idle,
                    });

                    info!(samples = samples.len(), "Recording stopped");
                    Ok(samples)
                })();
                let _ = reply.send(result);
            }
            AudioCommand::Shutdown => {
                break;
            }
        }
    }
    debug!("Audio thread shutting down");
}

/// cpal-based audio capture implementation.
///
/// Uses a dedicated audio thread to handle the non-Send Stream type.
pub struct CpalAudioManager {
    config: AudioConfig,
    state: Arc<AtomicAudioState>,
    event_sender: broadcast::Sender<AudioEvent>,
    current_level: Arc<AtomicU32>,
    selected_device_id: Arc<RwLock<Option<String>>>,
    recording_start: Mutex<Option<Instant>>,
    cmd_tx: mpsc::Sender<AudioCommand>,
    thread_handle: Mutex<Option<JoinHandle<()>>>,
}

impl CpalAudioManager {
    /// Create a new CpalAudioManager with default configuration.
    pub fn new() -> Result<Self, DomainError> {
        Self::with_config(AudioConfig::default())
    }

    /// Create a new CpalAudioManager with custom configuration.
    pub fn with_config(config: AudioConfig) -> Result<Self, DomainError> {
        let state = Arc::new(AtomicAudioState::default());
        let (event_sender, _) = broadcast::channel(64);
        let current_level = Arc::new(AtomicU32::new(0));
        let selected_device_id = Arc::new(RwLock::new(None));

        let (cmd_tx, cmd_rx) = mpsc::channel(16);

        // Clone Arcs for the thread
        let thread_config = config.clone();
        let thread_device_id = Arc::clone(&selected_device_id);
        let thread_state = Arc::clone(&state);
        let thread_event_sender = event_sender.clone();
        let thread_level = Arc::clone(&current_level);

        let thread_handle = thread::Builder::new()
            .name("audio-capture".to_string())
            .spawn(move || {
                audio_thread_main(
                    thread_config,
                    thread_device_id,
                    thread_state,
                    thread_event_sender,
                    thread_level,
                    cmd_rx,
                )
            })
            .map_err(|e| DomainError::AudioDevice {
                message: format!("Failed to spawn audio thread: {}", e),
            })?;

        info!(
            buffer_duration = config.buffer_duration_secs,
            sample_rate = config.sample_rate,
            "CpalAudioManager initialized"
        );

        Ok(Self {
            config,
            state,
            event_sender,
            current_level,
            selected_device_id,
            recording_start: Mutex::new(None),
            cmd_tx,
            thread_handle: Mutex::new(Some(thread_handle)),
        })
    }

    /// List available input devices with unique IDs.
    fn list_devices_internal(&self) -> Result<Vec<AudioDevice>, DomainError> {
        let host = cpal::default_host();
        let default_name = host.default_input_device().and_then(|d| d.name().ok());

        let devices = host.input_devices().map_err(|e| DomainError::AudioDevice {
            message: format!("Failed to enumerate devices: {}", e),
        })?;

        let mut result = Vec::new();
        let mut name_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

        for device in devices {
            if let Ok(name) = device.name() {
                // Generate unique ID by appending index for duplicate names
                let count = name_counts.entry(name.clone()).or_insert(0);
                let id = if *count == 0 {
                    name.clone()
                } else {
                    format!("{}:{}", name, count)
                };
                *count += 1;

                result.push(AudioDevice {
                    id,
                    name: name.clone(),
                    is_default: Some(&name) == default_name.as_ref(),
                });
            }
        }

        debug!(count = result.len(), "Listed input devices");
        Ok(result)
    }
}

impl Drop for CpalAudioManager {
    fn drop(&mut self) {
        // Send shutdown command
        let _ = self.cmd_tx.blocking_send(AudioCommand::Shutdown);

        // Wait for thread to finish
        if let Some(handle) = self.thread_handle.lock().take() {
            let _ = handle.join();
        }
    }
}

#[async_trait]
impl AudioManager for CpalAudioManager {
    async fn start_recording(&self) -> Result<(), DomainError> {
        let (reply_tx, reply_rx) = oneshot::channel();

        self.cmd_tx
            .send(AudioCommand::Start { reply: reply_tx })
            .await
            .map_err(|_| DomainError::AudioDevice {
                message: "Audio thread not running".to_string(),
            })?;

        let result = reply_rx.await.map_err(|_| DomainError::AudioDevice {
            message: "Audio thread did not respond".to_string(),
        })??;

        *self.recording_start.lock() = Some(Instant::now());
        Ok(result)
    }

    async fn stop_recording(&self) -> Result<AudioBuffer, DomainError> {
        let (reply_tx, reply_rx) = oneshot::channel();

        self.cmd_tx
            .send(AudioCommand::Stop { reply: reply_tx })
            .await
            .map_err(|_| DomainError::AudioDevice {
                message: "Audio thread not running".to_string(),
            })?;

        let samples = reply_rx.await.map_err(|_| DomainError::AudioDevice {
            message: "Audio thread did not respond".to_string(),
        })??;

        let duration = self
            .recording_start
            .lock()
            .take()
            .map(|t| t.elapsed().as_secs_f32())
            .unwrap_or(0.0);

        let mut buffer = AudioBuffer::with_capacity(self.config.sample_rate, samples.len());
        buffer.push_samples(&samples);

        info!(
            duration_secs = duration,
            samples = buffer.len(),
            "Recording stopped"
        );

        Ok(buffer)
    }

    fn state(&self) -> AudioState {
        self.state.load()
    }

    fn config(&self) -> AudioConfig {
        self.config.clone()
    }

    fn list_input_devices(&self) -> Result<Vec<AudioDevice>, DomainError> {
        self.list_devices_internal()
    }

    fn select_input_device(&self, device_id: Option<&str>) -> Result<(), DomainError> {
        if let Some(id) = device_id {
            let devices = self.list_devices_internal()?;
            if !devices.iter().any(|d| d.id == id) {
                return Err(DomainError::AudioDevice {
                    message: format!("Device not found: {}", id),
                });
            }
        }

        *self.selected_device_id.write() = device_id.map(String::from);
        info!(device_id = ?device_id, "Input device selected");
        Ok(())
    }

    fn subscribe(&self) -> broadcast::Receiver<AudioEvent> {
        self.event_sender.subscribe()
    }

    async fn recover(&self) -> Result<(), DomainError> {
        let current = self.state.load();
        if !current.can_recover() {
            return Err(DomainError::AudioStateTransition {
                from: current,
                to: AudioState::Recovering,
            });
        }

        // Transition to Recovering
        self.state.store(AudioState::Recovering);
        let _ = self.event_sender.send(AudioEvent::StateChanged {
            from: current,
            to: AudioState::Recovering,
        });

        // Attempt recovery with exponential backoff
        let max_attempts = self.config.max_recovery_attempts;
        for attempt in 1..=max_attempts {
            let delay_ms = 500 * (1 << (attempt - 1)); // 500ms, 1s, 2s
            tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;

            info!(attempt, max_attempts, delay_ms, "Recovery attempt");

            // Check if device is available
            match audio_processing::get_device(self.selected_device_id.read().as_deref()) {
                Ok(device) => {
                    let device_name = device.name().unwrap_or_else(|_| "Unknown".to_string());
                    self.state.store(AudioState::Idle);
                    let _ = self.event_sender.send(AudioEvent::RecoverySuccess {
                        device_name: device_name.clone(),
                    });
                    let _ = self.event_sender.send(AudioEvent::StateChanged {
                        from: AudioState::Recovering,
                        to: AudioState::Idle,
                    });
                    info!(device = %device_name, "Audio recovered successfully");
                    return Ok(());
                }
                Err(e) => {
                    warn!(attempt, error = %e, "Recovery attempt failed");
                }
            }
        }

        // All attempts failed
        self.state.store(AudioState::Error);
        let _ = self.event_sender.send(AudioEvent::RecoveryFailed {
            attempts: max_attempts,
            last_error: "Failed to recover audio device".to_string(),
        });
        let _ = self.event_sender.send(AudioEvent::StateChanged {
            from: AudioState::Recovering,
            to: AudioState::Error,
        });

        Err(DomainError::AudioDevice {
            message: format!("Recovery failed after {} attempts", max_attempts),
        })
    }

    fn current_duration(&self) -> f32 {
        self.recording_start
            .lock()
            .as_ref()
            .map(|t| t.elapsed().as_secs_f32())
            .unwrap_or(0.0)
    }

    fn current_level(&self) -> f32 {
        f32::from_bits(self.current_level.load(Ordering::Relaxed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_rms() {
        assert_eq!(audio_processing::calculate_rms(&[]), 0.0);
        assert_eq!(audio_processing::calculate_rms(&[0, 0, 0]), 0.0);

        let max_rms = audio_processing::calculate_rms(&[32767, 32767, 32767]);
        assert!((max_rms - 1.0).abs() < 0.001);

        let half_rms = audio_processing::calculate_rms(&[16384, -16384, 16384, -16384]);
        assert!(half_rms > 0.4 && half_rms < 0.6);
    }

    #[test]
    fn test_resample_same_rate() {
        let samples = vec![100, 200, 300, 400];
        let result = audio_processing::resample(&samples, 48000, 48000);
        assert_eq!(result, samples);
    }

    #[test]
    fn test_resample_downsample() {
        let samples: Vec<i16> = (0..48).map(|i| i * 100).collect();
        let result = audio_processing::resample(&samples, 48000, 16000);
        assert!(result.len() >= 15 && result.len() <= 17);
    }

    #[test]
    fn test_resample_upsample() {
        let samples = vec![0, 1000, 2000, 3000];
        let result = audio_processing::resample(&samples, 8000, 16000);
        assert!(result.len() >= 7 && result.len() <= 9);
    }
}
