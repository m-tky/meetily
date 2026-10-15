use std::sync::Arc;
use super::mixing::AudioMixerRingBuffer;
use super::pc_normalizer::PcLoudnessNormalizer;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use anyhow::Result;
use log::{debug, error, info, warn};
use crate::batch_audio_metric;
use super::batch_processor::AudioMetricsBatcher;
use rubato::{Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction};

use super::devices::AudioDevice;
use super::recording_state::{AudioChunk, AudioError, RecordingState, DeviceType};
use super::audio_processing::{audio_to_mono, LoudnessNormalizer, NoiseSuppressionProcessor, HighPassFilter};
use super::vad::{ContinuousVadProcessor};

/// How long a silence must last before the VAD closes a speech segment, and
/// therefore how long the audio clips handed to the ASR engine are.
///
/// Live-path policy: 500ms (matches the established Meetily Pro live policy).
/// The batch paths (`import.rs` / `retranscription.rs`) use 2000ms instead —
/// they have no latency requirement, so they optimize purely for ASR request
/// length. The live path cannot: with continuous audio (e.g. a podcast played
/// as system audio) a 2000ms redemption keeps one VAD segment open
/// indefinitely, withholds live transcript emission, and overruns the
/// accumulated-speech-buffer warning threshold. Bounded live segmentation
/// during continuous speech is tracked separately in #756.
const VAD_REDEMPTION_TIME_MS: u32 = 500;

/// Simple audio mixer without aggressive ducking
/// Combines mic + system audio with basic clipping prevention
struct ProfessionalAudioMixer;

impl ProfessionalAudioMixer {
    fn new(_sample_rate: u32) -> Self {
        Self
    }

    fn mix_window(&mut self, mic_window: &[f32], sys_window: &[f32]) -> Vec<f32> {
        // Handle different lengths (already padded by extract_window, but defensive)
        let max_len = mic_window.len().max(sys_window.len());
        let mut mixed = Vec::with_capacity(max_len);

        // Professional mixing with soft scaling to prevent distortion
        // Uses proportional scaling instead of hard clamping to avoid artifacts
        for i in 0..max_len {
            let mic = mic_window.get(i).copied().unwrap_or(0.0);
            let sys = sys_window.get(i).copied().unwrap_or(0.0);

            // Pre-scale system audio to 70% to leave headroom
            // This prevents constant soft scaling which can cause pumping artifacts
            // Mic is normalized to -23 LUFS (already optimal), system needs reduction
            let sys_scaled = sys * 1.0;
            let _mic_scaled = mic * 0.8;  // Reserved for future mic scaling

            // Sum without ducking - mic stays at full volume, system slightly reduced
            let sum = mic + sys_scaled;

            // CRITICAL FIX: Soft scaling prevents distortion artifacts
            // If the sum would exceed ±1.0, scale down PROPORTIONALLY
            // This avoids hard clipping distortion that sounds like "radio breaks"
            let sum_abs = sum.abs();
            let mixed_sample = if sum_abs > 1.0 {
                // Scale down to fit within ±1.0
                sum / sum_abs
            } else {
                sum
            };

            mixed.push(mixed_sample);
        }

        mixed
    }
}

// Numeric, opt-in transport telemetry only. Never logs samples or transcript.
// Read once per process so capture callbacks do not repeatedly inspect env.
fn transport_diagnostics_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var("MEETILY_AUDIO_TRANSPORT_DIAGNOSTICS").as_deref() == Ok("1"))
}

/// Simplified audio capture without broadcast channels
#[derive(Clone)]
pub struct AudioCapture {
    device: Arc<AudioDevice>,
    state: Arc<RecordingState>,
    sample_rate: u32,        // Original device sample rate
    channels: u16,
    chunk_counter: Arc<std::sync::atomic::AtomicU64>,
    device_type: DeviceType,
    recording_sender: Option<mpsc::UnboundedSender<AudioChunk>>,
    needs_resampling: bool,  // Flag if resampling is required
    // CRITICAL FIX: Persistent resampler to preserve energy across chunks
    resampler: Arc<std::sync::Mutex<Option<SincFixedIn<f32>>>>,
    // Buffering for variable-size chunks → fixed-size resampler input
    resampler_input_buffer: Arc<std::sync::Mutex<Vec<f32>>>,
    resampler_capture_start: Arc<std::sync::Mutex<Option<f64>>>,
    timing_warning: Arc<std::sync::atomic::AtomicBool>,
    capture_clock: Arc<std::sync::Mutex<super::capture_clock::CaptureClock>>,
    resampler_chunk_size: usize,  // Fixed chunk size for resampler (512 samples)
    // Audio enhancement processors (microphone only)
    noise_suppressor: Arc<std::sync::Mutex<Option<NoiseSuppressionProcessor>>>,
    high_pass_filter: Arc<std::sync::Mutex<Option<HighPassFilter>>>,
    // EBU R128 normalizer for microphone audio (per-device, stateful)
    normalizer: Arc<std::sync::Mutex<Option<LoudnessNormalizer>>>,
    // Separate bounded PC loudness controller; microphone policy is unchanged.
    system_normalizer: Arc<std::sync::Mutex<Option<PcLoudnessNormalizer>>>,
    // Note: Using global recording timestamp for synchronization
}

impl AudioCapture {
    pub fn new(
        device: Arc<AudioDevice>,
        state: Arc<RecordingState>,
        sample_rate: u32,
        channels: u16,
        device_type: DeviceType,
        recording_sender: Option<mpsc::UnboundedSender<AudioChunk>>,
    ) -> Self {
        // CRITICAL FIX: Detect if resampling is needed
        // Pipeline expects 48kHz, but Bluetooth devices often report 8kHz, 16kHz, or 44.1kHz
        const TARGET_SAMPLE_RATE: u32 = 48000;
        let needs_resampling = sample_rate != TARGET_SAMPLE_RATE;

        // Detect device kind (Bluetooth vs Wired) for adaptive processing
        // Use reasonable defaults for buffer size (512 samples is typical)
        let device_kind = super::device_detection::InputDeviceKind::detect(&device.name, 512, sample_rate);

        if needs_resampling {
            warn!(
                "⚠️ SAMPLE RATE MISMATCH DETECTED ⚠️"
            );
            warn!(
                "🔄 [{:?}] Audio device '{}' ({:?}) reports {} Hz (pipeline expects {} Hz)",
                device_type, device.name, device_kind, sample_rate, TARGET_SAMPLE_RATE
            );
            warn!(
                "🔄 Automatic resampling will be applied: {} Hz → {} Hz",
                sample_rate, TARGET_SAMPLE_RATE
            );

            // Log which resampling strategy will be used
            let ratio = TARGET_SAMPLE_RATE as f64 / sample_rate as f64;
            let strategy = if ratio >= 2.0 {
                "High-quality upsampling (sinc_len=512, Cubic interpolation)"
            } else if ratio >= 1.5 {
                "Moderate upsampling (sinc_len=384, Cubic)"
            } else if ratio > 1.0 {
                "Small upsampling (sinc_len=256, Linear)"
            } else if ratio <= 0.5 {
                "Anti-aliased downsampling (sinc_len=512, Cubic)"
            } else {
                "Moderate downsampling (sinc_len=384, Linear)"
            };
            info!("   Resampling strategy: {}", strategy);
        } else {
            info!(
                "✅ [{:?}] Audio device '{}' ({:?}) uses {} Hz (matches pipeline)",
                device_type, device.name, device_kind, sample_rate
            );
        }

        // Microphone filters and legacy normalizer remain unchanged.
        // PC loudness control is initialized separately below.
        let (noise_suppressor, high_pass_filter, normalizer) = if matches!(device_type, DeviceType::Microphone) {
            // Initialize noise suppression (RNNoise) at 48kHz - CONDITIONAL based on flag
            let ns = if super::ffmpeg_mixer::RNNOISE_APPLY_ENABLED {
                match NoiseSuppressionProcessor::new(TARGET_SAMPLE_RATE) {
                    Ok(processor) => {
                        info!("✅ RNNoise noise suppression ENABLED for microphone '{}' (10-15 dB reduction)", device.name);
                        Some(processor)
                    }
                    Err(e) => {
                        warn!("⚠️ Failed to create noise suppressor: {}, continuing without noise suppression", e);
                        None
                    }
                }
            } else {
                info!("ℹ️ RNNoise noise suppression DISABLED for microphone '{}' (flag: RNNOISE_APPLY_ENABLED=false)", device.name);
                info!("   Whisper handles noise well internally - RNNoise is optional");
                None
            };

            // Initialize high-pass filter (removes rumble below 80 Hz)
            let hpf = {
                let filter = HighPassFilter::new(TARGET_SAMPLE_RATE, 80.0);
                info!("✅ High-pass filter initialized for microphone '{}' (cutoff: 80 Hz)", device.name);
                Some(filter)
            };

            // Initialize EBU R128 normalizer (professional loudness standard)
            let norm = match LoudnessNormalizer::new(1, TARGET_SAMPLE_RATE) {
                Ok(normalizer) => {
                    info!("✅ EBU R128 normalizer initialized for microphone '{}' (target: -23 LUFS)", device.name);
                    Some(normalizer)
                }
                Err(e) => {
                    warn!("⚠️ Failed to create normalizer for microphone: {}, normalization disabled", e);
                    None
                }
            };

            (ns, hpf, norm)
        } else {
            // PC audio does not pass through microphone HPF/RNNoise.
            (None, None, None)
        };
        let system_normalizer = if matches!(device_type, DeviceType::System) {
            match PcLoudnessNormalizer::new(TARGET_SAMPLE_RATE) {
                Ok(normalizer) => {
                    info!("PC loudness normalization enabled: -23 LUFS, max gain +20dB, sample peak -1dBFS");
                    Some(normalizer)
                }
                Err(error) => {
                    warn!("PC loudness normalization unavailable: {}", error);
                    state.report_error(AudioError::ProcessingFailed);
                    None
                }
            }
        } else { None };

        // CRITICAL FIX: Initialize persistent resampler to preserve energy across chunks
        // Creating a new resampler per chunk causes energy amplification and incorrect output sizes
        // Use fixed chunk size of 512 samples with buffering for variable-size input
        const RESAMPLER_CHUNK_SIZE: usize = 512;

        let resampler = if needs_resampling {
            let ratio = TARGET_SAMPLE_RATE as f64 / sample_rate as f64;

            // Adaptive parameters based on sample rate ratio (same logic as resample_audio)
            let (sinc_len, interpolation_type, oversampling) = if ratio >= 2.0 {
                (512, SincInterpolationType::Cubic, 512)
            } else if ratio >= 1.5 {
                (384, SincInterpolationType::Cubic, 384)
            } else if ratio > 1.0 {
                (256, SincInterpolationType::Linear, 256)
            } else if ratio <= 0.5 {
                (512, SincInterpolationType::Cubic, 512)
            } else {
                (384, SincInterpolationType::Linear, 384)
            };

            let params = SincInterpolationParameters {
                sinc_len,
                f_cutoff: 0.95,
                interpolation: interpolation_type,
                oversampling_factor: oversampling,
                window: WindowFunction::BlackmanHarris2,
            };

            match SincFixedIn::<f32>::new(
                ratio,
                2.0,  // Maximum relative deviation
                params,
                RESAMPLER_CHUNK_SIZE,
                1,    // Mono
            ) {
                Ok(resampler) => {
                    info!("✅ Persistent resampler initialized for '{}' ({}Hz → {}Hz, chunk_size={})",
                          device.name, sample_rate, TARGET_SAMPLE_RATE, RESAMPLER_CHUNK_SIZE);
                    info!("   Buffering enabled for variable-size chunks (e.g., 320, 512, 1024, etc.)");
                    Some(resampler)
                }
                Err(e) => {
                    warn!("⚠️ Failed to create persistent resampler: {}, will use fallback", e);
                    None
                }
            }
        } else {
            None
        };

        Self {
            device,
            state,
            sample_rate,
            channels,
            chunk_counter: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            device_type,
            recording_sender,
            needs_resampling,
            resampler: Arc::new(std::sync::Mutex::new(resampler)),
            resampler_input_buffer: Arc::new(std::sync::Mutex::new(Vec::with_capacity(RESAMPLER_CHUNK_SIZE * 2))),
            resampler_capture_start: Arc::new(std::sync::Mutex::new(None)),
            timing_warning: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            capture_clock: Arc::new(std::sync::Mutex::new(super::capture_clock::CaptureClock::default())),
            resampler_chunk_size: RESAMPLER_CHUNK_SIZE,
            noise_suppressor: Arc::new(std::sync::Mutex::new(noise_suppressor)),
            high_pass_filter: Arc::new(std::sync::Mutex::new(high_pass_filter)),
            normalizer: Arc::new(std::sync::Mutex::new(normalizer)),
            system_normalizer: Arc::new(std::sync::Mutex::new(system_normalizer)),
            // Using global recording time for sync
        }
    }

    /// Process audio data directly from callback
    pub fn process_audio_data(&self, data: &[f32]) {
        self.process_audio_data_inner(data, None);
    }

    pub fn process_cpal_audio_data(&self, data: &[f32], info: &cpal::InputCallbackInfo) {
        // Keep the existing live CPAL route until the backend mapping passes
        // the shared-signal probe. This opt-in is only set by the private probe.
        static PRIVATE_TIMING: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        if !*PRIVATE_TIMING.get_or_init(|| std::env::var_os("MEETILY_PRIVATE_CPAL_TIMING_PROBE").as_deref() == Some(std::ffi::OsStr::new("1"))) {
            self.process_audio_data(data);
            return;
        }
        let stamp = info.timestamp();
        let at = stamp.callback.duration_since(&stamp.capture)
            .filter(|age| *age <= std::time::Duration::from_secs(10))
            .and_then(|age| std::time::Instant::now().checked_sub(age));
        self.process_audio_data_at(data, at);
    }

    /// `at` refers to the first input frame, not callback receipt. Native Pulse
    /// supplies the signed read-head latency; CPAL supplies capture/callback age.
    pub fn process_audio_data_at(&self, data: &[f32], at: Option<std::time::Instant>) {
        if !self.state.is_active() { return; }
        let measured = at.and_then(|t| self.state.capture_time_at(t));
        let capture_time = self.capture_clock.lock().unwrap().stamp(measured,
            data.len() / self.channels as usize, self.sample_rate, self.state.capture_generation());
        if transport_diagnostics_enabled() {
            info!("Audio clock measure: source={:?}, measured_s={:?}, frames={}, rate={}, mapped_s={:?}",
                self.device_type, measured, data.len() / self.channels as usize, self.sample_rate, capture_time);
        }
        if self.state.is_recording() && capture_time.is_none()
            && !self.timing_warning.swap(true, std::sync::atomic::Ordering::Relaxed) {
            warn!("Audio capture timing unavailable (source={:?}); no arrival-time fallback", self.device_type);
            self.state.report_error(AudioError::ProcessingFailed);
        }
        if capture_time.is_some() { self.timing_warning.store(false, std::sync::atomic::Ordering::Relaxed); }
        self.process_audio_data_inner(data, capture_time);
    }

    fn process_audio_data_inner(&self, data: &[f32], mut capture_timestamp: Option<f64>) {
        // Paused samples do not advance the recording's capture sample clock.
        if !self.state.is_active() {
            return;
        }

        // Convert to mono if needed
        let mut mono_data = if self.channels > 1 {
            audio_to_mono(data, self.channels)
        } else {
            data.to_vec()
        };

        // CRITICAL FIX: Resample to 48kHz if device uses different sample rate
        // This fixes Bluetooth devices (like Sony WH-1000XM4) that report 16kHz or 44.1kHz
        // Without this, audio is sped up 3x and VAD fails
        //
        // IMPORTANT: Uses PERSISTENT resampler with BUFFERING to preserve energy across chunks
        // Creating a new resampler per chunk causes energy amplification (173.5% RMS)
        // Buffering handles variable chunk sizes (320, 512, 1024, etc.) by accumulating to fixed 512-sample chunks
        const TARGET_SAMPLE_RATE: u32 = 48000;
        if self.needs_resampling {
            let before_len = mono_data.len();
            let before_rms = if !mono_data.is_empty() {
                (mono_data.iter().map(|&x| x * x).sum::<f32>() / mono_data.len() as f32).sqrt()
            } else {
                0.0
            };

            // Use persistent resampler with buffering to handle variable chunk sizes
            let mut resampled_output = Vec::new();
            let mut used_persistent_resampler = false;

            if let Ok(mut buffer_lock) = self.resampler_input_buffer.lock() {
                let mut time_lock = self.resampler_capture_start.lock().unwrap();
                let pending_len = buffer_lock.len();
                let buffer_time = if buffer_lock.is_empty() { capture_timestamp } else {
                    match (*time_lock, capture_timestamp) {
                        (Some(prior), Some(now)) if (prior + buffer_lock.len() as f64 / self.sample_rate as f64 - now).abs() <= 0.005 => Some(prior),
                        _ => None,
                    }
                };
                let mut consumed_input = 0usize;
                let mut output_delay = 0usize;
                // Add new samples to buffer
                buffer_lock.extend_from_slice(&mono_data);

                // Process complete chunks through the resampler
                if let Ok(mut resampler_lock) = self.resampler.lock() {
                    if let Some(ref mut resampler) = *resampler_lock {
                        used_persistent_resampler = true;
                        output_delay = resampler.output_delay();

                        // Process as many complete chunks as we have
                        while buffer_lock.len() >= self.resampler_chunk_size {
                            // Extract exactly chunk_size samples
                            let chunk: Vec<f32> = buffer_lock.drain(0..self.resampler_chunk_size).collect();
                            consumed_input += chunk.len();

                            // Rubato expects input as Vec<Vec<f32>> (one Vec per channel)
                            let waves_in = vec![chunk];

                            match resampler.process(&waves_in, None) {
                                Ok(mut waves_out) => {
                                    if let Some(output) = waves_out.pop() {
                                        resampled_output.extend_from_slice(&output);
                                    }
                                }
                                Err(e) => {
                                    warn!("⚠️ Persistent resampler processing failed: {}", e);
                                    used_persistent_resampler = false;
                                    break;
                                }
                            }
                        }
                        // Remaining samples in buffer will be processed in next iteration
                    }
                }
                *time_lock = if buffer_lock.is_empty() { None } else if consumed_input >= pending_len {
                    // Once the old/unknown prefix has been consumed, the
                    // remainder belongs to this input's valid capture range.
                    // Do not propagate one discontinuity as unknown forever.
                    capture_timestamp.map(|time| time + (consumed_input - pending_len) as f64 / self.sample_rate as f64)
                } else {
                    (*time_lock).map(|time| time + consumed_input as f64 / self.sample_rate as f64)
                };
                capture_timestamp = buffer_time.map(|time| time - output_delay as f64 / TARGET_SAMPLE_RATE as f64);
            }

            // CRITICAL: Only update mono_data if we got output from persistent resampler
            // If buffer is accumulating (< 512 samples), skip this chunk - data is safely buffered
            // and will be processed in next iteration with proper resampling
            let has_resampled_output = !resampled_output.is_empty();

            if has_resampled_output {
                mono_data = resampled_output;
            } else if !used_persistent_resampler {
                // Only fallback if persistent resampler is not available at all
                mono_data = super::audio_processing::resample_audio(
                    &mono_data,
                    self.sample_rate,
                    TARGET_SAMPLE_RATE,
                );
            } else {
                // Buffering: samples are accumulating in buffer, waiting for 512-sample chunk
                // Don't send partial/unprocessed data - return early
                // Audio is NOT lost - it's in the buffer and will be processed next iteration
                return;
            }

            // Log resampling only occasionally to avoid spam
            let chunk_id = self.chunk_counter.load(std::sync::atomic::Ordering::SeqCst);
            if chunk_id % 100 == 0 && has_resampled_output {
                let after_len = mono_data.len();
                let after_rms = if !mono_data.is_empty() {
                    (mono_data.iter().map(|&x| x * x).sum::<f32>() / mono_data.len() as f32).sqrt()
                } else {
                    0.0
                };
                let ratio = TARGET_SAMPLE_RATE as f64 / self.sample_rate as f64;
                let rms_preservation = if before_rms > 0.0 { (after_rms / before_rms) * 100.0 } else { 100.0 };

                let buffer_size = if let Ok(buf) = self.resampler_input_buffer.lock() {
                    buf.len()
                } else {
                    0
                };

                info!(
                    "🔄 [{:?}] Persistent buffered resampler: {}Hz → {}Hz (ratio: {:.2}x)",
                    self.device_type,
                    self.sample_rate,
                    TARGET_SAMPLE_RATE,
                    ratio
                );
                info!(
                    "   Chunk {}: {} → {} samples, RMS preservation: {:.1}%, buffer: {}",
                    chunk_id,
                    before_len,
                    after_len,
                    rms_preservation,
                    buffer_size
                );
            }
        }

        // AUDIO ENHANCEMENT PIPELINE (Microphone Only)
        // Processing order is critical: high-pass → noise suppression → normalization
        // This ensures noise is removed before being amplified by the normalizer
        if matches!(self.device_type, DeviceType::Microphone) {
            // STEP 1: Apply high-pass filter to remove low-frequency rumble (< 80 Hz)
            if let Ok(mut hpf_lock) = self.high_pass_filter.lock() {
                if let Some(ref mut filter) = *hpf_lock {
                    mono_data = filter.process(&mono_data);
                }
            }

            // STEP 2: Apply RNNoise noise suppression (10-15 dB reduction) - CONDITIONAL
            if super::ffmpeg_mixer::RNNOISE_APPLY_ENABLED {
                if let Ok(mut ns_lock) = self.noise_suppressor.lock() {
                    if let Some(ref mut suppressor) = *ns_lock {
                        let before_len = mono_data.len();
                        mono_data = suppressor.process(&mono_data);
                        let after_len = mono_data.len();

                        // CRITICAL MONITORING: Track buffer health
                        let chunk_id = self.chunk_counter.load(std::sync::atomic::Ordering::SeqCst);
                        if chunk_id % 100 == 0 {
                            let buffered = suppressor.buffered_samples();
                            let length_delta = (before_len as i32 - after_len as i32).abs();

                            debug!("🔇 Noise suppression health: in={}, out={}, delta={}, buffered={}, RMS={:.4}",
                                   before_len, after_len, length_delta, buffered,
                                   if !mono_data.is_empty() {
                                       (mono_data.iter().map(|&x| x * x).sum::<f32>() / mono_data.len() as f32).sqrt()
                                   } else { 0.0 });

                            // WARN if accumulating samples (potential latency buildup)
                            if buffered > 1000 {
                                warn!("⚠️ RNNoise accumulating samples: {} buffered (potential latency issue!)",
                                      buffered);
                            }

                            // WARN if significant length mismatch
                            if length_delta > 50 {
                                warn!("⚠️ RNNoise length mismatch: input={} output={} (delta={})",
                                      before_len, after_len, length_delta);
                            }
                        }
                    }
                }
            }

            // STEP 3: Apply EBU R128 normalization (professional loudness standard)
            if let Ok(mut normalizer_lock) = self.normalizer.lock() {
                if let Some(ref mut normalizer) = *normalizer_lock {
                    mono_data = normalizer.normalize_loudness(&mono_data);

                    // Log normalization occasionally for debugging
                    let chunk_id = self.chunk_counter.load(std::sync::atomic::Ordering::SeqCst);
                    if chunk_id % 200 == 0 && !mono_data.is_empty() {
                        let rms = (mono_data.iter().map(|&x| x * x).sum::<f32>() / mono_data.len() as f32).sqrt();
                        let peak = mono_data.iter().map(|&x| x.abs()).fold(0.0f32, f32::max);
                        debug!("🎤 After normalization chunk {}: RMS={:.4}, Peak={:.4}", chunk_id, rms, peak);
                    }
                }
            }
        } else if matches!(self.device_type, DeviceType::System) {
            let mut normalizer = self.system_normalizer.lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(normalizer) = normalizer.as_mut() {
                mono_data = normalizer.process(&mono_data);
            }
        }

        // Create audio chunk with stream-specific timestamp (get ID first for logging)
        let chunk_id = self.chunk_counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        // Per-source loudness control is complete; no post-mix gain is applied.

        // DIAGNOSTIC: Log audio levels for debugging (especially mic issues)
        // if chunk_id % 100 == 0 && !mono_data.is_empty() {
        //     let raw_rms = (mono_data.iter().map(|&x| x * x).sum::<f32>() / mono_data.len() as f32).sqrt();
        //     let raw_peak = mono_data.iter().map(|&x| x.abs()).fold(0.0f32, f32::max);

        //         info!("🎙️ [{:?}] Chunk {} - Raw: RMS={:.6}, Peak={:.6}",
        //               self.device_type, chunk_id, raw_rms, raw_peak);

        //     // Warn if microphone is completely silent
        //     if matches!(self.device_type, DeviceType::Microphone) && raw_rms == 0.0 && raw_peak == 0.0 {
        //         warn!("⚠️ Microphone producing ZERO audio - check permissions or hardware!");
        //     }
        // }
        // else if chunk_id % 100 == 0 && matches!(self.device_type, DeviceType::System) {
        //     let raw_rms = (mono_data.iter().map(|&x| x * x).sum::<f32>() / mono_data.len() as f32).sqrt();
        //     let raw_peak = mono_data.iter().map(|&x| x.abs()).fold(0.0f32, f32::max);
        //     info!("🔊 [{:?}] Chunk {} - Raw: RMS={:.6}, Peak={:.6}",
        //       self.device_type, chunk_id, raw_rms, raw_peak);
            
        //     // Warn if system audio is completely silent
        //     if raw_rms == 0.0 && raw_peak == 0.0 {
        //         warn!("⚠️ System audio producing ZERO audio - check permissions or hardware!");
        //     }
        // }

        // Use global recording timestamp for proper synchronization
        let timestamp = self.state.get_recording_duration().unwrap_or(0.0);
        if transport_diagnostics_enabled() {
            info!("Audio transport handoff: source={:?}, chunk={}, samples={}, at_s={:.6}",
                self.device_type, chunk_id, mono_data.len(), timestamp);
            info!("Audio transport capture: source={:?}, chunk={}, start_s={:?}", self.device_type, chunk_id, capture_timestamp);
        }

        // Processed per-source audio goes to the common mixing path.
        // Use 48kHz if we resampled, otherwise use original rate
        let audio_chunk = AudioChunk { capture_timestamp,
            data: mono_data,  // Mono48k, per-source loudness controlled
            sample_rate: if self.needs_resampling { 48000 } else { self.sample_rate },
            timestamp,
            chunk_id,
            device_type: self.device_type.clone(),
        };

        // NOTE: Raw audio is NOT sent to recording saver to prevent echo
        // Only the mixed audio (from AudioPipeline) is saved to file (see pipeline.rs:726-736)
        // This ensures we only record once: mic + system properly mixed
        // Individual raw streams go only to the transcription pipeline below

        // Send to processing pipeline for transcription
        if let Err(e) = self.state.send_audio_chunk(audio_chunk) {
            // Check if this is the "pipeline not ready" error
            if e.to_string().contains("Audio pipeline not ready") {
                // This is expected during initialization, just log it as debug
                debug!("Audio pipeline not ready yet, skipping chunk {}", chunk_id);
                return;
            }

            warn!("Failed to send audio chunk: {}", e);
            // More specific error handling based on failure reason
            let error = if e.to_string().contains("channel closed") {
                AudioError::ChannelClosed
            } else if e.to_string().contains("full") {
                AudioError::BufferOverflow
            } else {
                AudioError::ProcessingFailed
            };
            self.state.report_error(error);
        } else {
            debug!("Sent audio chunk {} ({} samples)", chunk_id, data.len());
        }
    }

    /// Handle stream errors with enhanced disconnect detection
    pub fn handle_stream_error(&self, error: cpal::StreamError) {
        self.capture_clock.lock().unwrap().reset();
        error!("Audio stream error for {}: {}", self.device.name, error);

        let error_str = error.to_string().to_lowercase();

        // Enhanced error detection for device disconnection
        let audio_error = if error_str.contains("device is no longer available")
            || error_str.contains("device not found")
            || error_str.contains("device disconnected")
            || error_str.contains("no such device")
            || error_str.contains("device unavailable")
            || error_str.contains("device removed")
        {
            warn!("🔌 Device disconnect detected for: {}", self.device.name);
            AudioError::DeviceDisconnected
        } else if error_str.contains("permission") || error_str.contains("access denied") {
            AudioError::PermissionDenied
        } else if error_str.contains("channel closed") {
            AudioError::ChannelClosed
        } else if error_str.contains("stream") && error_str.contains("failed") {
            AudioError::StreamFailed
        } else {
            warn!("Unknown audio error: {}", error);
            AudioError::StreamFailed
        };

        self.state.report_error(audio_error);
    }
}

/// VAD-driven audio processing pipeline
/// Uses Voice Activity Detection to segment speech in real-time and send only speech to Whisper
pub struct AudioPipeline {
    receiver: mpsc::UnboundedReceiver<AudioChunk>,
    transcription_sender: mpsc::UnboundedSender<AudioChunk>,
    state: Arc<RecordingState>,
    mic_vad_processor: Option<ContinuousVadProcessor>,
    system_vad_processor: Option<ContinuousVadProcessor>,
    sample_rate: u32,
    chunk_id_counter: u64,
    // Performance optimization: reduce logging frequency
    last_summary_time: std::time::Instant,
    processed_chunks: u64,
    // Smart batching for audio metrics
    metrics_batcher: Option<AudioMetricsBatcher>,
    // PROFESSIONAL AUDIO MIXING: Ring buffer + RMS-based mixer
    ring_buffer: AudioMixerRingBuffer,
    timed_ring: Option<super::timed_mixing::TimedMixer>,
    timing_unavailable: [bool; 2],
    mixer: ProfessionalAudioMixer,
    // Recording sender for pre-mixed audio
    recording_sender_for_mixed: Option<mpsc::UnboundedSender<AudioChunk>>,
}

impl AudioPipeline {
    pub fn new(
        receiver: mpsc::UnboundedReceiver<AudioChunk>,
        transcription_sender: mpsc::UnboundedSender<AudioChunk>,
        state: Arc<RecordingState>,
        target_chunk_duration_ms: u32,
        sample_rate: u32,
        mic_device_name: String,
        mic_device_kind: super::device_detection::InputDeviceKind,
        system_device_name: String,
        system_device_kind: super::device_detection::InputDeviceKind,
        mic_enabled: bool,
        system_enabled: bool,
    ) -> Result<Self> {
        // Log device characteristics for adaptive buffering
        info!("🎛️ AudioPipeline initializing with device characteristics:");
        info!("   Mic: '{}' ({:?}) - Buffer: {:?}",
              mic_device_name, mic_device_kind, mic_device_kind.buffer_timeout());
        info!("   System: '{}' ({:?}) - Buffer: {:?}",
              system_device_name, system_device_kind, system_device_kind.buffer_timeout());

        // Device kind information can be used for adaptive buffering in the future
        // For now, we log it for monitoring and potential optimization
        let _ = (mic_device_name, mic_device_kind, system_device_name, system_device_kind);

        // Create VAD processor. The VAD processor handles 48kHz->16kHz resampling
        // internally.
        //
        // Redemption time is how long a silence must last before the VAD closes a
        // speech segment, so it decides how long the audio clips handed to the ASR
        // engine are. Conversational speech pauses constantly for breath and
        // mid-sentence thought, and every pause longer than this becomes a segment
        // boundary and therefore a separate transcription request.
        //
        // This was 400ms, which fragmented a 26-minute meeting into 322 requests with
        // a median length of 3.5s. Whisper is a fixed 30-second-window model: below
        // that it zero-pads the window and leans on its language-model prior, which
        // was trained on web subtitles, so short clips come back as memorised
        // boilerplate ("subscribe to the channel", "thank you") instead of speech.
        // Measured on a real recording, 47% of segment boundaries sat in the
        // 0.42-0.75s range that a longer redemption bridges.
        //
        // 500ms is the live-path policy (see the constant's doc comment). The
        // batch value (2000ms, `import.rs`/`retranscription.rs`) was tried here
        // first, but under continuous system audio it kept a VAD segment open
        // indefinitely and withheld live transcript emission, so live and batch
        // deliberately diverge. Bounded live segments under continuous speech
        // are tracked in #756.
        // Independent state/context per input; neither lane is a person's identity.
        let mic_vad_processor = mic_enabled.then(||
            ContinuousVadProcessor::new(sample_rate, VAD_REDEMPTION_TIME_MS)).transpose()?;
        let system_vad_processor = system_enabled.then(||
            ContinuousVadProcessor::new(sample_rate, VAD_REDEMPTION_TIME_MS)).transpose()?;
        info!(
            "VAD-driven pipeline: segments dispatched per speech burst (redemption_time={}ms)",
            VAD_REDEMPTION_TIME_MS
        );

        // Initialize professional audio mixing components
        let ring_buffer = AudioMixerRingBuffer::new(sample_rate, mic_enabled, system_enabled);
        let mixer = ProfessionalAudioMixer::new(sample_rate);

        // Note: target_chunk_duration_ms is ignored - VAD controls segmentation now
        let _ = target_chunk_duration_ms;

        Ok(Self {
            receiver,
            transcription_sender,
            state,
            mic_vad_processor,
            system_vad_processor,
            sample_rate,
            chunk_id_counter: 0,
            // Performance optimization: reduce logging frequency
            last_summary_time: std::time::Instant::now(),
            processed_chunks: 0,
            // Initialize metrics batcher for smart batching
            metrics_batcher: Some(AudioMetricsBatcher::new()),
            // Initialize professional audio mixing
            ring_buffer,
            timed_ring: None,
            timing_unavailable: [false; 2],
            mixer,
            recording_sender_for_mixed: None,  // Will be set by manager
        })
    }

    /// Run the VAD-driven audio processing pipeline
    pub async fn run(mut self) -> Result<()> {
        info!("VAD-driven audio pipeline started - segments sent in real-time based on speech detection");

        // CRITICAL FIX: Continue processing until channel is closed, not based on recording state
        // This ensures ALL chunks are processed during shutdown, fixing premature meeting completion
        // Previous bug: Loop checked `while self.state.is_recording()` which caused early exit when
        // stop_recording() was called, losing flush signals and remaining chunks in the pipeline
        loop {
            // Receive audio chunks with timeout
            match tokio::time::timeout(
                std::time::Duration::from_millis(50), // Shorter timeout for responsiveness
                self.receiver.recv()
            ).await {
                Ok(Some(chunk)) => {
                    // PERFORMANCE: Check for flush signal (special chunk with ID >= u64::MAX - 10)
                    // Multiple flush signals may be sent to ensure processing
                    if chunk.chunk_id >= u64::MAX - 10 {
                        debug!("Received Stop marker; defer flush until all queued audio is drained");
                        // Flushing a partial ring before later queued source chunks would
                        // pad their interval prematurely. Channel close is the final boundary.
                        continue;
                    }

                    // PERFORMANCE OPTIMIZATION: Eliminate per-chunk logging overhead
                    // Logging in hot paths causes severe performance degradation
                    self.processed_chunks += 1;

                    // Smart batching: collect metrics instead of logging every chunk
                    if let Some(ref batcher) = self.metrics_batcher {
                        let avg_level = chunk.data.iter().map(|&x| x.abs()).sum::<f32>() / chunk.data.len() as f32;
                        let duration_ms = chunk.data.len() as f64 / chunk.sample_rate as f64 * 1000.0;

                        batch_audio_metric!(
                            Some(batcher),
                            chunk.chunk_id,
                            chunk.data.len(),
                            duration_ms,
                            avg_level
                        );
                    }

                    // CRITICAL: Log summary only every 200 chunks OR every 60 seconds (99.5% reduction)
                    // This eliminates I/O overhead in the audio processing hot path
                    // Use performance-optimized debug macro that compiles to nothing in release builds
                    if self.processed_chunks % 200 == 0 || self.last_summary_time.elapsed().as_secs() >= 60 {
                        perf_debug!("Pipeline processed {} chunks, current chunk: {} ({} samples)",
                                   self.processed_chunks, chunk.chunk_id, chunk.data.len());
                        self.last_summary_time = std::time::Instant::now();
                    }

                    let source = chunk.device_type.clone();
                    if transport_diagnostics_enabled() {
                        info!("Audio transport receive: source={:?}, chunk={}, samples={}, at_s={:.6}, handoff_s={:.6}",
                            source, chunk.chunk_id, chunk.data.len(),
                            self.state.get_recording_duration().unwrap_or(0.0), chunk.timestamp);
                    }
                    let appended = if let Some(ring) = self.timed_ring.as_mut() {
                        let index = if source == DeviceType::Microphone { 0 } else { 1 };
                        let a = ring.append(chunk.device_type, chunk.capture_timestamp, chunk.data);
                        if a.invalid {
                            if !self.timing_unavailable[index] {
                                warn!("Audio capture timestamp unknown/invalid (source={:?}); no guessed placement", source);
                                self.state.report_error(AudioError::ProcessingFailed);
                            }
                            self.timing_unavailable[index] = true;
                        } else { self.timing_unavailable[index] = false; }
                        if a.gap > 0 || a.overlap > 0 {
                            warn!("Audio capture time discontinuity (source={:?}): gap_samples={}, overlap_samples={}", source, a.gap, a.overlap);
                            self.state.report_error(AudioError::StreamFailed);
                        }
                        super::mixing::AppendOutcome { late_samples: a.late,
                            overflow_samples: a.overflow }
                    } else {
                        self.ring_buffer.add_samples(chunk.device_type, chunk.data)
                    };
                    if appended.late_samples > 0 {
                        warn!("Discarded {} late audio samples from an already emitted interval (source={:?})", appended.late_samples, source);
                    }
                    if appended.overflow_samples > 0 {
                        warn!("Audio ring overflow: {} samples lost; preserving the interval as silence", appended.overflow_samples);
                        self.state.report_error(AudioError::BufferOverflow);
                    }
                    self.drain_mixed_audio(false)?;
                }
                Ok(None) => {
                    info!("Audio pipeline: sender closed after processing {} chunks", self.processed_chunks);
                    break;
                }
                Err(_) => {
                    // Check the bounded input wait even when no new callback arrives.
                    self.drain_mixed_audio(false)?;
                }
            }
        }

        // Flush any remaining VAD segments
        self.flush_remaining_audio()?;

        info!("VAD-driven audio pipeline ended");
        Ok(())
    }

    fn next_mixer_window(&mut self, finishing: bool) -> Option<super::mixing::MixerWindow> {
        let now = std::time::Instant::now();
        if let Some(ring) = self.timed_ring.as_mut() {
            ring.next_window(now, finishing).map(|w| super::mixing::MixerWindow {
                mic: w.mic, system: w.system, start_sample: w.start_sample,
                degraded_started: w.degraded_started,
            })
        } else { self.ring_buffer.next_window(now, finishing) }
    }

    /// Drain ready windows through the same recording and transcription paths.
    /// The output sample cursor, not callback arrival time, supplies timestamps.
    fn drain_mixed_audio(&mut self, finishing: bool) -> Result<()> {
        while let Some(window) = self.next_mixer_window(finishing) {
            if window.degraded_started {
                warn!("Audio input stalled for 1200ms; padding missing samples and discarding late arrivals");
                self.state.report_error(AudioError::StreamFailed);
            }
            let timestamp = window.start_sample as f64 / self.sample_rate as f64;
            if transport_diagnostics_enabled() {
                info!("Audio transport mix: start_sample={}, samples={}, at_s={:.6}, finishing={}, degraded_started={}",
                    window.start_sample, window.mic.len(), self.state.get_recording_duration().unwrap_or(0.0),
                    finishing, window.degraded_started);
            }
            let mixed = self.mixer.mix_window(&window.mic, &window.system);
            // Feed every aligned window, including silence, to each selected lane.
            // Both VAD sample cursors therefore share the recording timeline. Never
            // transcribe the mix: simultaneous sources remain separate requests.
            for (kind, samples) in [(DeviceType::Microphone, &window.mic),
                                    (DeviceType::System, &window.system)] {
                let processor = match kind {
                    DeviceType::Microphone => self.mic_vad_processor.as_mut(),
                    DeviceType::System => self.system_vad_processor.as_mut(),
                };
                if let Some(processor) = processor {
                    match processor.process_audio(samples) {
                        Ok(segments) => self.dispatch_source_segments(segments, kind),
                        Err(e) => {
                            warn!("Source VAD error ({:?}): {}", kind, e);
                            self.state.report_error(AudioError::ProcessingFailed);
                        }
                    }
                }
            }
            if let Some(ref sender) = self.recording_sender_for_mixed {
                let recording_chunk = AudioChunk { capture_timestamp: None,
                    data: mixed,
                    sample_rate: self.sample_rate,
                    timestamp,
                    chunk_id: self.chunk_id_counter,
                    device_type: DeviceType::Microphone,
                };
                if sender.send(recording_chunk).is_err() {
                    self.state.report_error(AudioError::ChannelClosed);
                    return Err(anyhow::anyhow!("Mixed recording receiver closed"));
                }
            }
        }
        Ok(())
    }

    fn dispatch_source_segments(&mut self, segments: Vec<super::vad::SpeechSegment>, kind: DeviceType) {
        for segment in segments {
            if segment.samples.len() < 800 { continue; }
            let chunk = AudioChunk { capture_timestamp: None,
                data: segment.samples, sample_rate: 16000,
                timestamp: segment.start_timestamp_ms / 1000.0,
                chunk_id: self.chunk_id_counter, device_type: kind.clone(),
            };
            if let Err(e) = self.transcription_sender.send(chunk) {
                warn!("Failed to send source VAD segment ({:?}): {}", kind, e);
                self.state.report_error(AudioError::ChannelClosed);
            } else { self.chunk_id_counter += 1; }
        }
    }

    fn flush_remaining_audio(&mut self) -> Result<()> {
        info!("Flushing source audio (processed {} chunks)", self.processed_chunks);
        // Drain the exact shared short tail before independently flushing both VADs.
        self.drain_mixed_audio(true)?;
        for kind in [DeviceType::Microphone, DeviceType::System] {
            let processor = match kind {
                DeviceType::Microphone => self.mic_vad_processor.as_mut(),
                DeviceType::System => self.system_vad_processor.as_mut(),
            };
            if let Some(processor) = processor {
                match processor.flush() {
                    Ok(segments) => self.dispatch_source_segments(segments, kind),
                    Err(e) => {
                        warn!("Failed to flush source VAD ({:?}): {}", kind, e);
                        self.state.report_error(AudioError::ProcessingFailed);
                    }
                }
            }
        }
        Ok(())
    }

}

#[cfg(test)]
#[path = "source_transcription_tests.rs"]
mod source_transcription_tests;

/// Simple audio pipeline manager
pub struct AudioPipelineManager {
    pipeline_handle: Option<JoinHandle<Result<()>>>,
    audio_sender: Option<mpsc::UnboundedSender<AudioChunk>>,
}

impl AudioPipelineManager {
    pub fn new() -> Self {
        Self {
            pipeline_handle: None,
            audio_sender: None,
        }
    }

    /// Start the audio pipeline with device information for adaptive buffering
    pub fn start(
        &mut self,
        state: Arc<RecordingState>,
        transcription_sender: mpsc::UnboundedSender<AudioChunk>,
        target_chunk_duration_ms: u32,
        sample_rate: u32,
        recording_sender: Option<mpsc::UnboundedSender<AudioChunk>>,
        mic_device_name: String,
        mic_device_kind: super::device_detection::InputDeviceKind,
        system_device_name: String,
        system_device_kind: super::device_detection::InputDeviceKind,
        mic_enabled: bool,
        system_enabled: bool,
    ) -> Result<()> {
        // Log device information for adaptive buffering
        info!("🎙️ Starting pipeline with device info:");
        info!("   Microphone: '{}' ({:?})", mic_device_name, mic_device_kind);
        info!("   System Audio: '{}' ({:?})", system_device_name, system_device_kind);

        // Create audio processing channel
        let (audio_sender, audio_receiver) = mpsc::unbounded_channel::<AudioChunk>();


        // Create and start pipeline with device information for adaptive mixing
        #[cfg(target_os = "linux")]
        let native_timing = (!mic_enabled || mic_device_name.starts_with(super::capture::pulse::INPUT_PREFIX))
            && (!system_enabled || system_device_name.starts_with(super::capture::pulse::PREFIX));
        let mut pipeline = AudioPipeline::new(
            audio_receiver,
            transcription_sender,
            state.clone(),
            target_chunk_duration_ms,
            sample_rate,
            mic_device_name,
            mic_device_kind,
            system_device_name,
            system_device_kind,
            mic_enabled,
            system_enabled,
        )?;
        pipeline.ring_buffer = AudioMixerRingBuffer::with_window(sample_rate, mic_enabled, system_enabled, super::mixing::LIVE_WINDOW_MS);
        info!("Audio mixing window: {}ms (input wait unchanged: 1200ms)", super::mixing::LIVE_WINDOW_MS);
        #[cfg(target_os = "linux")]
        {
            if native_timing {
                pipeline.timed_ring = Some(super::timed_mixing::TimedMixer::with_window(mic_enabled, system_enabled, super::mixing::LIVE_WINDOW_MS));
                info!("Linux capture-time synchronization enabled; unknown timestamps are not inferred");
            }
        }
        state.set_audio_sender(audio_sender.clone());

        // CRITICAL FIX: Connect recording sender to receive pre-mixed audio
        // This ensures both mic AND system audio are captured in recordings
        pipeline.recording_sender_for_mixed = recording_sender;

        let handle = tokio::spawn(async move {
            pipeline.run().await
        });

        self.pipeline_handle = Some(handle);
        self.audio_sender = Some(audio_sender);

        info!("Audio pipeline manager started with mixed audio recording");
        Ok(())
    }

    /// Stop the audio pipeline
    pub async fn stop(&mut self) -> Result<()> {
        // Drop the sender to close the pipeline
        self.audio_sender = None;

        // Wait for pipeline to finish
        if let Some(handle) = self.pipeline_handle.take() {
            match handle.await {
                Ok(result) => result,
                Err(e) => {
                    error!("Pipeline task failed: {}", e);
                    Ok(())
                }
            }
        } else {
            Ok(())
        }
    }

    /// Force immediate flush of accumulated audio and stop pipeline
    /// PERFORMANCE CRITICAL: Eliminates 30+ second shutdown delays
    pub async fn force_flush_and_stop(&mut self) -> Result<()> {
        info!("🚀 Force flushing pipeline - processing ALL accumulated audio immediately");

        // If we have a sender, send a special flush signal first
        if let Some(sender) = &self.audio_sender {
            // Create a special flush chunk to trigger immediate processing
            let flush_chunk = AudioChunk { capture_timestamp: None,
                data: vec![], // Empty data signals flush
                sample_rate: 16000,
                timestamp: 0.0,
                chunk_id: u64::MAX, // Special ID to indicate flush
                device_type: super::recording_state::DeviceType::Microphone,
            };

            if let Err(e) = sender.send(flush_chunk) {
                warn!("Failed to send flush signal: {}", e);
            } else {
                info!("📤 Sent flush signal to pipeline");

                // PERFORMANCE OPTIMIZATION: Reduced wait time from 50ms to 20ms
                // Pipeline should process flush signal very quickly
                tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;

                // Send multiple flush signals to ensure the pipeline catches it
                // This aggressive approach eliminates shutdown delay issues
                for i in 0..3 {
                    let additional_flush = AudioChunk { capture_timestamp: None,
                        data: vec![],
                        sample_rate: 16000,
                        timestamp: 0.0,
                        chunk_id: u64::MAX - (i as u64),
                        device_type: super::recording_state::DeviceType::Microphone,
                    };
                    let _ = sender.send(additional_flush);
                }

                info!("📤 Sent additional flush signals for reliability");
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            }
        }

        // Now stop normally
        self.stop().await
    }
}

impl Default for AudioPipelineManager {
    fn default() -> Self {
        Self::new()
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    pub(super) fn synthetic_pipeline(mic: bool, system: bool) -> (
        AudioPipeline,
        mpsc::UnboundedReceiver<AudioChunk>,
        mpsc::UnboundedReceiver<AudioChunk>,
    ) {
        let (_, input) = mpsc::unbounded_channel();
        let (transcription, transcript_rx) = mpsc::unbounded_channel();
        let (recording, recording_rx) = mpsc::unbounded_channel();
        let kind = super::super::device_detection::InputDeviceKind::Unknown;
        let mut pipeline = AudioPipeline::new(
            input, transcription, RecordingState::new(), 0, 48000,
            "synthetic mic".into(), kind, "synthetic system".into(), kind, mic, system,
        ).unwrap();
        pipeline.recording_sender_for_mixed = Some(recording);
        (pipeline, recording_rx, transcript_rx)
    }

    #[tokio::test]
    async fn short_live_windows_keep_sample_timestamps_and_stop_tail() {
        for timed in [false, true] {
            let (mut p, mut recording, _) = synthetic_pipeline(true, true);
            p.ring_buffer = AudioMixerRingBuffer::with_window(48000, true, true, super::super::mixing::LIVE_WINDOW_MS);
            if timed {
                let mut r = super::super::timed_mixing::TimedMixer::with_window(true, true, super::super::mixing::LIVE_WINDOW_MS);
                r.append(DeviceType::Microphone, Some(0.0), vec![0.0; 24001]);
                r.append(DeviceType::System, Some(0.0), vec![0.0; 24001]);
                p.timed_ring = Some(r);
            } else {
                p.ring_buffer.add_samples(DeviceType::Microphone, vec![0.0; 24001]);
                p.ring_buffer.add_samples(DeviceType::System, vec![0.0; 24001]);
            }
            p.drain_mixed_audio(false).unwrap();
            p.flush_remaining_audio().unwrap();
            let mut count = 0;
            let mut sizes = Vec::new();
            while let Ok(chunk) = recording.try_recv() {
                assert_eq!(chunk.timestamp, count as f64 / 48000.0);
                count += chunk.data.len();
                sizes.push(chunk.data.len());
            }
            assert_eq!(sizes, vec![5760, 5760, 5760, 5760, 961]);
            assert_eq!(count, 24001);
            assert_eq!(p.state.get_error_count(), 0);
        }
    }

    #[tokio::test]
    async fn capture_time_pipeline_preserves_startup_offset_and_exact_stop_tail() {
        let (mut p, mut recording, _) = synthetic_pipeline(true, true);
        let mut ring = super::super::timed_mixing::TimedMixer::new(true, true);
        ring.append(DeviceType::System, Some(0.6), vec![0.0;48000]);
        ring.append(DeviceType::Microphone, Some(1.85), vec![0.0;4800]);
        p.timed_ring = Some(ring);
        p.drain_mixed_audio(false).unwrap();
        p.flush_remaining_audio().unwrap();
        let mut count = 0;
        while let Ok(chunk) = recording.try_recv() {
            assert_eq!(chunk.timestamp, count as f64 / 48000.0);
            count += chunk.data.len();
        }
        assert_eq!(count, 93600);
        assert_eq!(p.state.get_error_count(), 0);
    }

    #[tokio::test]
    async fn capture_time_pipeline_unknown_timestamp_warns_without_guessing_placement() {
        let (mut p, mut recording, _) = synthetic_pipeline(true, false);
        let (tx, rx) = mpsc::unbounded_channel(); p.receiver = rx;
        p.timed_ring = Some(super::super::timed_mixing::TimedMixer::new(true, false));
        let state = p.state.clone();
        for id in 0..2 { tx.send(AudioChunk { data: vec![0.0;480], sample_rate:48000,
            timestamp: 99.0, capture_timestamp:None, chunk_id:id, device_type:DeviceType::Microphone }).unwrap(); }
        drop(tx); p.run().await.unwrap();
        assert_eq!(state.get_error_count(),1);
        assert!(recording.try_recv().is_err());
    }

    #[tokio::test]
    async fn resampler_timestamp_recovers_after_unknown_prefix_is_consumed() {
        let state = RecordingState::new();
        let (tx, mut rx) = mpsc::unbounded_channel();
        state.set_audio_sender(tx);
        state.start_recording().unwrap();
        let at = std::time::Instant::now();
        let device = Arc::new(AudioDevice::new("synthetic resampled mic".into(), super::super::devices::DeviceType::Input));
        let capture = AudioCapture::new(device, state, 44100, 1, DeviceType::Microphone, None);
        capture.process_audio_data_at(&vec![0.0; 600], Some(at));
        assert!(rx.try_recv().unwrap().capture_timestamp.is_some());
        capture.process_audio_data_at(&vec![0.0; 600], Some(at + std::time::Duration::from_millis(300)));
        assert!(rx.try_recv().unwrap().capture_timestamp.is_none());
        capture.process_audio_data_at(&vec![0.0; 600], Some(at + std::time::Duration::from_secs_f64(0.3 + 600.0 / 44100.0)));
        assert!(rx.try_recv().unwrap().capture_timestamp.is_some());
    }

    #[tokio::test]
    async fn native_capture_timestamp_is_not_handoff_time() {
        let state = RecordingState::new(); let (tx, mut rx) = mpsc::unbounded_channel();
        state.set_audio_sender(tx); state.start_recording().unwrap();
        let device = Arc::new(AudioDevice::new("synthetic native".into(), super::super::devices::DeviceType::Output));
        let capture = AudioCapture::new(device, state.clone(), 48000, 1, DeviceType::System, None);
        let at = std::time::Instant::now() + std::time::Duration::from_millis(120);
        capture.process_audio_data_at(&[0.0;480], Some(at));
        let chunk = rx.try_recv().unwrap();
        assert!(chunk.capture_timestamp.unwrap() >= 0.12);
        assert!(chunk.timestamp < 0.1);
        assert_eq!(chunk.data.len(),480);
        state.stop_recording();
    }

    #[tokio::test]
    async fn test_stop_short_tail_goes_through_recording_and_real_vad_once() {
        let (mut p, mut recording, mut transcripts) = synthetic_pipeline(false, true);
        p.ring_buffer.add_samples(DeviceType::System, vec![0.0;24001]);
        p.drain_mixed_audio(false).unwrap();
        assert!(recording.try_recv().is_err());
        p.flush_remaining_audio().unwrap();
        let saved = recording.try_recv().unwrap();
        assert_eq!(saved.data.len(),24001);
        assert_eq!(saved.sample_rate,48000);
        assert_eq!(saved.timestamp,0.0);
        assert!(transcripts.try_recv().is_err(), "silence must not emit a speech segment");
        p.flush_remaining_audio().unwrap();
        assert!(recording.try_recv().is_err(), "Stop must not duplicate the short tail");
    }

    #[tokio::test]
    async fn test_delayed_dual_input_creates_one_recording_window() {
        let (mut p, mut recording, _) = synthetic_pipeline(true, true);
        p.ring_buffer.add_samples(DeviceType::Microphone, vec![0.0;28800]);
        p.drain_mixed_audio(false).unwrap();
        assert!(recording.try_recv().is_err());
        p.ring_buffer.add_samples(DeviceType::System, vec![0.0;28800]);
        p.drain_mixed_audio(false).unwrap();
        assert_eq!(recording.try_recv().unwrap().data.len(),28800);
        assert!(recording.try_recv().is_err());
        p.flush_remaining_audio().unwrap();
        assert!(recording.try_recv().is_err());
    }

    #[tokio::test]
    async fn test_output_sample_timestamps_and_exact_final_length() {
        let (mut p, mut recording, _) = synthetic_pipeline(true, false);
        p.ring_buffer.add_samples(DeviceType::Microphone, vec![0.0;30001]);
        p.drain_mixed_audio(false).unwrap();
        let first = recording.try_recv().unwrap();
        p.flush_remaining_audio().unwrap();
        let tail = recording.try_recv().unwrap();
        assert_eq!(first.timestamp,0.0);
        assert_eq!(tail.timestamp,0.6);
        assert_eq!(first.data.len()+tail.data.len(),30001);
        assert_eq!(tail.data.len(),1201);
    }

    #[tokio::test]
    async fn test_single_selected_source_does_not_wait() {
        for mic in [true, false] {
            let (mut p, mut recording, _) = synthetic_pipeline(mic, !mic);
            let kind = if mic {DeviceType::Microphone} else {DeviceType::System};
            p.ring_buffer.add_samples(kind, vec![0.0;28800]);
            p.drain_mixed_audio(false).unwrap();
            assert_eq!(recording.try_recv().unwrap().data.len(),28800);
        }
    }

    #[tokio::test]
    async fn test_mixed_recording_channel_failure_is_not_silent() {
        let (mut p, recording, _) = synthetic_pipeline(false, true);
        drop(recording);
        p.ring_buffer.add_samples(DeviceType::System, vec![0.0;24000]);
        assert!(p.flush_remaining_audio().is_err());
    }

    #[tokio::test]
    async fn test_queued_stop_marker_does_not_flush_before_later_source() {
        let (mut p, mut recording, _) = synthetic_pipeline(true, true);
        let (tx, rx) = mpsc::unbounded_channel();
        p.receiver = rx;
        for (kind, count, id) in [
            (DeviceType::Microphone,28800,0),
            (DeviceType::Microphone,0,u64::MAX),
            (DeviceType::System,28800,1),
        ] {
            tx.send(AudioChunk {capture_timestamp: None,data:vec![0.0;count],sample_rate:48000,
                timestamp:99.0,chunk_id:id,device_type:kind}).unwrap();
        }
        drop(tx);
        p.run().await.unwrap();
        let saved = recording.try_recv().unwrap();
        assert_eq!(saved.data.len(),28800);
        assert_eq!(saved.timestamp,0.0, "callback time must not set output timeline");
        assert!(recording.try_recv().is_err());
    }

    #[tokio::test]
    async fn test_channel_close_drains_exact_short_tail() {
        let (mut p, mut recording, _) = synthetic_pipeline(false, true);
        let (tx, rx) = mpsc::unbounded_channel();
        p.receiver = rx;
        tx.send(AudioChunk {capture_timestamp: None,data:vec![0.0;24001],sample_rate:48000,
            timestamp:99.0,chunk_id:0,device_type:DeviceType::System}).unwrap();
        drop(tx);
        p.run().await.unwrap();
        assert_eq!(recording.try_recv().unwrap().data.len(),24001);
        assert!(recording.try_recv().is_err());
    }

    #[tokio::test]
    async fn test_idle_receive_timeout_fills_once_and_discards_late_interval() {
        let (mut p, mut recording, _) = synthetic_pipeline(true, true);
        let (tx, rx) = mpsc::unbounded_channel();
        p.receiver = rx;
        tx.send(AudioChunk {capture_timestamp: None,data:vec![0.0;28800],sample_rate:48000,
            timestamp:99.0,chunk_id:0,device_type:DeviceType::Microphone}).unwrap();
        let task = tokio::spawn(p.run());
        let saved = tokio::time::timeout(std::time::Duration::from_secs(3),recording.recv())
            .await.unwrap().unwrap();
        assert_eq!(saved.data.len(),28800);
        tx.send(AudioChunk {capture_timestamp: None,data:vec![0.0;28800],sample_rate:48000,
            timestamp:100.0,chunk_id:1,device_type:DeviceType::System}).unwrap();
        drop(tx);
        tokio::time::timeout(std::time::Duration::from_secs(3),task).await.unwrap().unwrap().unwrap();
        assert!(recording.try_recv().is_err(), "late interval must not elongate recording");
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn test_synthetic_speech_tail_matches_direct_vad_and_saved_samples() {
        // Offline synthesized fixture to stdout only: no playback/capture/ASR.
        // Requires shell.nix's espeak-ng and ffmpeg, never opens an audio device.
        use std::io::Write;
        use std::process::{Command, Stdio};
        let speech = Command::new("espeak-ng").args([
            "--stdout", "-v", "en", "-s", "160",
            "This is a synthetic audio test. The final sentence must remain in the recording.",
        ]).output().expect("synthetic fixture requires espeak-ng");
        assert!(speech.status.success());
        let mut decoder = Command::new("ffmpeg").args([
            "-v", "error", "-f", "wav", "-i", "pipe:0", "-ac", "1", "-ar", "48000",
            "-f", "f32le", "pipe:1",
        ]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
            .spawn().expect("synthetic fixture requires ffmpeg");
        let mut input = decoder.stdin.take().unwrap();
        let writer = std::thread::spawn(move || input.write_all(&speech.stdout));
        let decoded = decoder.wait_with_output().unwrap();
        writer.join().unwrap().unwrap();
        assert!(decoded.status.success());
        assert_eq!(decoded.stdout.len()%4,0);
        let mut samples = vec![0.0;96000]; // two seconds of synthetic silence
        samples.extend(decoded.stdout.chunks_exact(4).map(|b|f32::from_le_bytes(b.try_into().unwrap())));
        assert!(samples.len()%28800>0, "fixture must leave a short ring tail");
        for window_ms in [600, super::super::mixing::LIVE_WINDOW_MS] {
        let window_samples = 48 * window_ms as usize;
        let mut reference = ContinuousVadProcessor::new(48000,VAD_REDEMPTION_TIME_MS).unwrap();
        let mut expected = Vec::new();
        for chunk in samples.chunks(window_samples) {
            expected.extend(reference.process_audio(chunk).unwrap());
        }
        expected.extend(reference.flush().unwrap());
        expected.retain(|s|s.samples.len()>=800);
        assert!(!expected.is_empty(), "fixture must exercise speech-segment delivery");

        let (mut p, mut recording, mut transcripts) = synthetic_pipeline(false,true);
        p.ring_buffer = AudioMixerRingBuffer::with_window(48000, false, true, window_ms);
        for chunk in samples.chunks(window_samples) {
            p.ring_buffer.add_samples(DeviceType::System,chunk.to_vec());
            p.drain_mixed_audio(false).unwrap();
        }
        p.flush_remaining_audio().unwrap();
        let mut saved = Vec::new();
        while let Ok(chunk) = recording.try_recv() {saved.extend(chunk.data);}
        assert_eq!(saved,samples, "synthetic speech must retain its exact short ring tail");
        let mut actual = Vec::new();
        while let Ok(chunk) = transcripts.try_recv() {actual.push(chunk);}
        assert_eq!(actual.len(),expected.len());
        for (a,e) in actual.iter().zip(expected.iter()) {
            assert_eq!(a.data,e.samples);
            assert_eq!(a.timestamp,e.start_timestamp_ms/1000.0);
            assert_eq!(a.sample_rate,16000);
        }
        }
    }

    #[test]
    fn test_system_capture_uses_bounded_normalization_without_mic_filters() {
        let state = RecordingState::new();
        state.start_recording().unwrap();
        let (tx, mut rx) = mpsc::unbounded_channel();
        state.set_audio_sender(tx);
        let device = Arc::new(AudioDevice::new("synthetic PC".into(), super::super::devices::DeviceType::Output));
        let capture = AudioCapture::new(device,state,48000,1,DeviceType::System,None);
        assert!(capture.system_normalizer.lock().unwrap().is_some());
        assert!(capture.normalizer.lock().unwrap().is_none());
        assert!(capture.high_pass_filter.lock().unwrap().is_none());
        assert!(capture.noise_suppressor.lock().unwrap().is_none());
        let samples: Vec<f32> = (0..96000).map(|i|
            0.002 * (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 48000.0).sin()).collect();
        let mut reference = PcLoudnessNormalizer::new(48000).unwrap();
        let expected = reference.process(&samples);
        capture.process_audio_data(&samples);
        let chunk = rx.try_recv().unwrap();
        assert_eq!(chunk.data,expected);
        assert_eq!(chunk.sample_rate,48000);
        assert_eq!(chunk.device_type,DeviceType::System);
    }

    #[test]
    fn test_microphone_keeps_legacy_normalizer_separate() {
        let device = Arc::new(AudioDevice::new("synthetic mic".into(), super::super::devices::DeviceType::Input));
        let capture = AudioCapture::new(device,RecordingState::new(),48000,1,DeviceType::Microphone,None);
        assert!(capture.normalizer.lock().unwrap().is_some());
        assert!(capture.high_pass_filter.lock().unwrap().is_some());
        assert!(capture.system_normalizer.lock().unwrap().is_none());
    }

    #[test]
    fn test_live_vad_redemption_matches_pro_policy() {
        // Live uses the established 500ms pause policy; it does not bound
        // uninterrupted speech. Batch import/retranscription use 2000ms.
        // See #679 and #756.
        assert_eq!(VAD_REDEMPTION_TIME_MS, 500);
    }
}
