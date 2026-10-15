//! PRIVATE artificial-audio transport probe; no ASR, host capture or app profile.
//! Run only via scripts/probe_dual_latency.py, which owns both sinks and playback.
#[cfg(target_os = "linux")]
mod recording_state {
    pub use app_lib::audio::recording_state::DeviceType;
}
#[cfg(target_os = "linux")]
#[path = "../src/audio/mixing.rs"]
mod mixing;
#[cfg(target_os = "linux")]
#[path = "../src/audio/timed_mixing.rs"]
mod timed_mixing;
#[cfg(all(target_os = "linux", test))]
#[path = "../src/audio/capture_clock.rs"]
mod capture_clock;
#[cfg(target_os = "linux")]
struct ProbeRing {
    legacy: mixing::AudioMixerRingBuffer,
    timed: Option<timed_mixing::TimedMixer>,
    invalid: usize,
    gaps: u64,
    overlaps: usize,
}
#[cfg(target_os = "linux")]
impl ProbeRing {
    fn add_samples(
        &mut self,
        kind: recording_state::DeviceType,
        data: Vec<f32>,
        time: Option<f64>,
    ) -> mixing::AppendOutcome {
        if let Some(r) = self.timed.as_mut() {
            let a = r.append(kind, time, data);
            self.invalid += usize::from(a.invalid);
            self.gaps += a.gap;
            self.overlaps += a.overlap;
            mixing::AppendOutcome {
                late_samples: a.late,
                overflow_samples: a.overflow,
            }
        } else {
            self.legacy.add_samples(kind, data)
        }
    }
    fn next_window(
        &mut self,
        now: std::time::Instant,
        finishing: bool,
    ) -> Option<mixing::MixerWindow> {
        if let Some(r) = self.timed.as_mut() {
            r.next_window(now, finishing).map(|w| mixing::MixerWindow {
                mic: w.mic,
                system: w.system,
                start_sample: w.start_sample,
                degraded_started: w.degraded_started,
            })
        } else {
            self.legacy.next_window(now, finishing)
        }
    }
}

#[cfg(target_os = "linux")]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    use anyhow::ensure;
    use app_lib::audio::{
        capture::pulse::{MonitorLatencyMode, PulseCapture},
        devices::{AudioDevice, DeviceType as DeviceKind},
        pipeline::AudioCapture,
        recording_state::{DeviceType, RecordingState},
        stream::AudioStream,
    };
    use serde_json::json;
    use std::{
        path::PathBuf,
        sync::Arc,
        time::{Duration, Instant},
    };

    let mut args = std::env::args().skip(1);
    let root = PathBuf::from(
        args.next()
            .ok_or_else(|| anyhow::anyhow!("private root required"))?,
    )
    .canonicalize()?;
    let mode = args
        .next()
        .ok_or_else(|| anyhow::anyhow!("mode required"))?;
    ensure!(
        root.join(".private-latency-test").is_file(),
        "private test marker required"
    );
    ensure!(
        std::env::var("PULSE_SERVER")? == format!("unix:{}", root.join("pulse").display()),
        "private server required"
    );
    ensure!(
        std::env::var("ALSA_CONFIG_PATH")? == root.join("alsa.conf").to_string_lossy(),
        "private ALSA required"
    );
    ensure!(
        mode == "fragment" || mode == "adjust" || mode == "timed" || mode == "native",
        "invalid mode"
    );
    let policy = if mode != "fragment" {
        MonitorLatencyMode::AdjustSource
    } else {
        MonitorLatencyMode::FragmentOnly
    };
    struct ProbeLogger;
    impl log::Log for ProbeLogger {
        fn enabled(&self, _: &log::Metadata) -> bool {
            true
        }
        fn log(&self, record: &log::Record) {
            eprintln!("{}", record.args());
        }
        fn flush(&self) {}
    }
    static LOGGER: ProbeLogger = ProbeLogger;
    let _ = log::set_logger(&LOGGER);
    log::set_max_level(log::LevelFilter::Info);
    let window_ms: u32 = std::env::var("MEETILY_PRIVATE_PROBE_WINDOW_MS")
        .unwrap_or_else(|_| "600".into())
        .parse()?;
    ensure!(
        window_ms == 600 || window_ms == 120,
        "unsupported probe window"
    );
    let state = RecordingState::new();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    state.set_audio_sender(tx);
    state.start_recording()?;
    let epoch = Instant::now();
    let mic = AudioStream::create(
        Arc::new(AudioDevice::new(
            if mode == "native" {
                "pulse-input:fixture_native_mic".into()
            } else {
                "fixture_mic".into()
            },
            DeviceKind::Input,
        )),
        state.clone(),
        DeviceType::Microphone,
        None,
    )
    .await?;
    let pc_device = Arc::new(AudioDevice::new(
        "pulse-monitor:fixture_pc.monitor".into(),
        DeviceKind::Output,
    ));
    let capture = AudioCapture::new(pc_device, state.clone(), 48000, 1, DeviceType::System, None);
    let errors = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let errors_copy = errors.clone();
    let error = move |_| {
        errors_copy.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    };
    let pc = if mode == "timed" || mode == "native" {
        PulseCapture::start_timed(
            "pulse-monitor:fixture_pc.monitor",
            move |data, at| capture.process_audio_data_at(data, at),
            error,
        )?
    } else {
        PulseCapture::start_with_latency_mode(
            "pulse-monitor:fixture_pc.monitor",
            policy,
            move |data| capture.process_audio_data(data),
            error,
        )?
    };
    std::fs::write(root.join("ready"), b"idle-private-capture-ready\n")?;
    let mut arrivals = [Vec::<f64>::new(), Vec::<f64>::new()];
    let mut inputs = [Vec::<f32>::new(), Vec::<f32>::new()];
    let mut ends = [Vec::<usize>::new(), Vec::<usize>::new()];
    let mut mixed = [Vec::<f32>::new(), Vec::<f32>::new()];
    let mut lag = [Vec::<f64>::new(), Vec::<f64>::new()];
    let mut ring = ProbeRing {
        legacy: mixing::AudioMixerRingBuffer::with_window(48000, true, true, window_ms),
        timed: if mode == "timed" || mode == "native" {
            Some(timed_mixing::TimedMixer::with_window(true, true, window_ms))
        } else {
            None
        },
        invalid: 0,
        gaps: 0,
        overlaps: 0,
    };
    let mut late = 0;
    let mut overflow = 0;
    let mut stalls = 0;
    let mut first_output = None;
    let mut output_times = Vec::new();
    // Use the real production ring and sample positions, but no VAD/model or
    // encoder: output timing is measured before those independent stages.
    loop {
        if root.join("stop").exists() || epoch.elapsed() > Duration::from_secs(90) {
            mic.stop()?;
            drop(pc);
            state.stop_recording();
            // Drain queued Stop tail below. Pulse owner is now joined.
            while let Ok(chunk) = rx.try_recv() {
                let role = if chunk.device_type == DeviceType::Microphone {
                    0
                } else {
                    1
                };
                arrivals[role].push(chunk.timestamp);
                inputs[role].extend_from_slice(&chunk.data);
                ends[role].push(inputs[role].len());
                let outcome =
                    ring.add_samples(chunk.device_type, chunk.data, chunk.capture_timestamp);
                late += outcome.late_samples;
                overflow += outcome.overflow_samples;
            }
            while let Some(w) = ring.next_window(Instant::now(), true) {
                mixed[0].extend(w.mic);
                mixed[1].extend(w.system);
            }
            break;
        }
        if let Ok(Some(chunk)) = tokio::time::timeout(Duration::from_millis(10), rx.recv()).await {
            let role = if chunk.device_type == DeviceType::Microphone {
                0
            } else {
                1
            };
            let now = epoch.elapsed().as_secs_f64();
            arrivals[role].push(chunk.timestamp); // capture handoff, NOT hardware time
            lag[role].push((now - chunk.timestamp).max(0.0));
            inputs[role].extend_from_slice(&chunk.data);
            ends[role].push(inputs[role].len());
            let outcome = ring.add_samples(chunk.device_type, chunk.data, chunk.capture_timestamp);
            late += outcome.late_samples;
            overflow += outcome.overflow_samples;
        }
        while let Some(w) = ring.next_window(Instant::now(), false) {
            stalls += usize::from(w.degraded_started);
            let now = epoch.elapsed().as_secs_f64();
            first_output.get_or_insert(now);
            output_times.push(json!({"output_at_s":now,"start_sample":w.start_sample}));
            mixed[0].extend(w.mic);
            mixed[1].extend(w.system);
        }
    }
    fn quantile(v: &[f64], q: f64) -> f64 {
        if v.is_empty() {
            return 0.0;
        }
        let mut v = v.to_vec();
        v.sort_by(f64::total_cmp);
        v[((v.len() - 1) as f64 * q).round() as usize]
    }
    fn onsets(data: &[f32]) -> Vec<f64> {
        let mut active = false;
        let mut quiet = 10;
        let mut out = Vec::new();
        for (i, block) in data.chunks(480).enumerate() {
            let rms =
                (block.iter().map(|v| (v * v) as f64).sum::<f64>() / block.len() as f64).sqrt();
            if rms > 0.01 {
                if !active && quiet >= 5 {
                    out.push(i as f64 * 0.01);
                }
                active = true;
                quiet = 0;
            } else {
                quiet += 1;
                if quiet >= 5 {
                    active = false;
                }
            }
        }
        out
    }
    let roles: Vec<_> = (0..2).map(|i| {
        let gaps: Vec<_> = arrivals[i].windows(2).map(|w| (w[1]-w[0])*1000.0).collect();
        let input_onsets = onsets(&inputs[i]);
        let onset_handoffs: Vec<_> = input_onsets.iter().map(|s| {
            let sample = (s * 48000.0).round() as usize;
            ends[i].iter().position(|&end| end > sample).map(|j| arrivals[i][j])
        }).collect();
        json!({"callbacks":arrivals[i].len(),"first_handoff_s":arrivals[i].first(),
            "handoff_gap_p50_ms":quantile(&gaps,0.5),"handoff_gap_p95_ms":quantile(&gaps,0.95),"handoff_gap_max_ms":quantile(&gaps,1.0),
            "channel_lag_p95_ms":quantile(&lag[i],0.95)*1000.0,
            "samples":inputs[i].len(),"input_onsets_s":input_onsets,"onset_handoff_s":onset_handoffs,"output_onsets_s":onsets(&mixed[i])})
    }).collect();
    let mut native_input_lifecycle = false;
    if mode == "native" && root.join("test-input-lifecycle").exists() {
        use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
        let output = std::process::Command::new("pactl")
            .args(["--format=json", "list", "modules"])
            .output()?;
        ensure!(
            output.status.success(),
            "private module introspection failed"
        );
        let modules: serde_json::Value = serde_json::from_slice(&output.stdout)?;
        let module = modules
            .as_array()
            .unwrap()
            .iter()
            .find(|m| {
                m["name"] == "module-remap-source"
                    && m["argument"]
                        .as_str()
                        .unwrap_or("")
                        .contains("source_name=fixture_native_mic")
            })
            .ok_or_else(|| anyhow::anyhow!("owned private input module missing"))?;
        // Pulse 17's JSON module listing omits the index entirely. Resolve
        // the owned module through the public short listing, not array order.
        let short = std::process::Command::new("pactl")
            .args(["list", "short", "modules"])
            .output()?;
        ensure!(short.status.success(), "private module index lookup failed");
        let listing = String::from_utf8(short.stdout)?;
        let indices: Vec<_> = listing
            .lines()
            .filter_map(|line| {
                let fields: Vec<_> = line.split('\t').collect();
                (fields.len() >= 3
                    && fields[1] == "module-remap-source"
                    && fields[2] == module["argument"].as_str().unwrap_or("")
                    && !fields[0].is_empty()
                    && fields[0].chars().all(|c| c.is_ascii_digit()))
                .then(|| fields[0].to_owned())
            })
            .collect();
        ensure!(
            indices.len() == 1,
            "owned private module index is ambiguous"
        );
        let index = &indices[0];
        let master = if module["argument"]
            .as_str()
            .unwrap()
            .contains("master=fixture_pc.monitor")
        {
            "master=fixture_pc.monitor"
        } else {
            "master=fixture_mic.monitor"
        };
        // Keep the private null sink active for the removal/reconnect test.
        // An idle suspended remap source need not deliver callbacks at all.
        struct PrivatePlayback(std::process::Child);
        impl Drop for PrivatePlayback {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
        let sink = if master == "master=fixture_pc.monitor" {
            "fixture_pc"
        } else {
            "fixture_mic"
        };
        let _playback = PrivatePlayback(
            std::process::Command::new("paplay")
                .args([
                    "--raw",
                    "--format=float32le",
                    "--rate=48000",
                    "--channels=1",
                    "--device",
                    sink,
                ])
                .arg(root.join("pulses.f32"))
                .spawn()?,
        );
        let callbacks = Arc::new(AtomicUsize::new(0));
        let seen = callbacks.clone();
        let lost = Arc::new(AtomicBool::new(false));
        let flag = lost.clone();
        let stream = PulseCapture::start_input_timed(
            "pulse-input:fixture_native_mic",
            move |data, time| {
                if time.is_some() && data.iter().all(|v| v.is_finite()) {
                    seen.fetch_add(1, Ordering::Relaxed);
                }
            },
            move |_| {
                flag.store(true, Ordering::Relaxed);
            },
        )?;
        let began = Instant::now();
        while callbacks.load(Ordering::Relaxed) == 0 && began.elapsed() < Duration::from_secs(5) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        ensure!(
            callbacks.load(Ordering::Relaxed) > 0,
            "private input did not deliver timed samples"
        );
        ensure!(
            std::process::Command::new("pactl")
                .args(["unload-module", index.as_str()])
                .status()?
                .success(),
            "private removal failed"
        );
        let began = Instant::now();
        while !lost.load(Ordering::Relaxed) && began.elapsed() < Duration::from_secs(3) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        ensure!(
            lost.load(Ordering::Relaxed),
            "native input removal was hidden"
        );
        drop(stream);
        ensure!(
            PulseCapture::start_input_timed("pulse-input:fixture_native_mic", |_, _| {}, |_| {})
                .is_err(),
            "missing native input silently fell back"
        );
        let result = std::process::Command::new("pactl")
            .args([
                "load-module",
                "module-remap-source",
                master,
                "source_name=fixture_native_mic",
                "channels=1",
            ])
            .output()?;
        ensure!(result.status.success(), "private recreate failed");
        let resumed = Arc::new(AtomicUsize::new(0));
        let seen = resumed.clone();
        let stream = PulseCapture::start_input_timed(
            "pulse-input:fixture_native_mic",
            move |data, time| {
                if time.is_some() && data.iter().all(|v| v.is_finite()) {
                    seen.fetch_add(1, Ordering::Relaxed);
                }
            },
            |_| {},
        )?;
        let began = Instant::now();
        while resumed.load(Ordering::Relaxed) == 0 && began.elapsed() < Duration::from_secs(5) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        ensure!(
            resumed.load(Ordering::Relaxed) > 0,
            "recreated input did not deliver timed samples"
        );
        let began = Instant::now();
        drop(stream);
        ensure!(
            began.elapsed() < Duration::from_secs(1),
            "native input stop blocked"
        );
        native_input_lifecycle = true;
    }
    let report = json!({"mode":mode,"rate":48000,"roles_mic_pc":roles,"first_mix_output_s":first_output,
        "output_windows":output_times,"mixed_samples":mixed[0].len(),"late_samples":late,"overflow_samples":overflow,
        "stall_episodes":stalls,"capture_errors":errors.load(std::sync::atomic::Ordering::Relaxed),
        "invalid_timestamps":ring.invalid,"gap_samples":ring.gaps,"overlap_samples":ring.overlaps,
        "backend_capture_time_used":mode=="timed" || mode=="native",
        "native_input_lifecycle_passed":native_input_lifecycle,
        "hardware_timestamp_measurement":false,"asr_used":false,"cloud":false});
    std::fs::write(
        root.join("probe-result.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    Ok(())
}
#[cfg(not(target_os = "linux"))]
fn main() {
    panic!("Linux private test only");
}
