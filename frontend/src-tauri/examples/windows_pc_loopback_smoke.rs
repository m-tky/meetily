//! Explicitly authorized synthetic PC-only capture. Never opens a microphone,
//! ASR, network, app profile or meeting DB. Run only with the external session guard.
#[cfg(not(windows))]
fn main() {
    eprintln!("Windows-only, explicitly authorized synthetic loopback test");
}

#[cfg(windows)]
mod probe {
    use anyhow::{bail, ensure, Context, Result};
    use app_lib::audio::{
        devices::{AudioDevice, DeviceType},
        AudioStreamManager, RecordingDeviceType, RecordingState,
    };
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use std::{
        io::Write,
        path::{Path, PathBuf},
        sync::{
            atomic::{AtomicBool, AtomicUsize, Ordering},
            Arc,
        },
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };

    fn guarded(lease: &Path) -> Result<()> {
        let s =
            std::fs::read_to_string(lease).context("session guard absent; no capture permitted")?;
        let parts: Vec<_> = s.split_whitespace().collect();
        ensure!(
            parts.len() == 2 && parts[0].parse::<u32>()? == std::process::id(),
            "wrong session guard owner"
        );
        let stamp = parts[1].parse::<u128>()?;
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
        ensure!(
            now >= stamp && now - stamp < 1500,
            "session guard heartbeat expired"
        );
        Ok(())
    }
    fn tone(t: f64) -> f32 {
        // Three quiet, distinct test tones, separated by silence; no speech.
        for (begin, hz) in [(0.5, 523.25), (1.7, 739.99), (2.9, 997.0)] {
            let x = t - begin;
            if (0.0..0.8).contains(&x) {
                let fade = (x / 0.01).min((0.8 - x) / 0.01).min(1.0);
                return (0.05 * fade * (std::f64::consts::TAU * hz * x).sin()) as f32;
            }
        }
        0.0
    }
    fn save_wav(path: &Path, samples: &[f32]) -> Result<()> {
        let size = u32::try_from(samples.len() * 4)?;
        let mut f = std::io::BufWriter::new(
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)?,
        );
        f.write_all(b"RIFF")?;
        f.write_all(&(50 + size).to_le_bytes())?;
        f.write_all(b"WAVEfmt ")?;
        f.write_all(&18u32.to_le_bytes())?;
        f.write_all(&3u16.to_le_bytes())?;
        f.write_all(&1u16.to_le_bytes())?;
        f.write_all(&48000u32.to_le_bytes())?;
        f.write_all(&192000u32.to_le_bytes())?;
        f.write_all(&4u16.to_le_bytes())?;
        f.write_all(&32u16.to_le_bytes())?;
        f.write_all(&0u16.to_le_bytes())?;
        f.write_all(b"fact")?;
        f.write_all(&4u32.to_le_bytes())?;
        f.write_all(&(samples.len() as u32).to_le_bytes())?;
        f.write_all(b"data")?;
        f.write_all(&size.to_le_bytes())?;
        for x in samples {
            f.write_all(&x.to_le_bytes())?;
        }
        f.flush()?;
        f.get_ref().sync_all()?;
        Ok(())
    }
    pub async fn run() -> Result<()> {
        let args: Vec<_> = std::env::args_os().collect();
        ensure!(
            args.len() == 3,
            "usage: windows_pc_loopback_smoke OWNED_OUTPUT_DIRECTORY LEASE_FILE"
        );
        let dir = PathBuf::from(&args[1]);
        let lease = PathBuf::from(&args[2]);
        ensure!(
            dir.join(".synthetic-pc-only").is_file(),
            "owned synthetic test marker missing"
        );
        ensure!(
            !dir.join("system-captured.wav").exists(),
            "refusing to overwrite prior capture"
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        while guarded(&lease).is_err() {
            ensure!(Instant::now() < deadline, "session guard not armed");
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let host = cpal::host_from_id(cpal::HostId::Wasapi)?;
        // Output enumeration only: no input/microphone device is even requested.
        let output = host
            .default_output_device()
            .context("no default WASAPI render endpoint")?;
        let name = output.name()?;
        let config = output.default_output_config()?;
        ensure!(
            config.sample_format() == cpal::SampleFormat::F32,
            "probe requires Windows shared F32 mix"
        );
        let rate = config.sample_rate().0;
        let channels = config.channels() as usize;
        let frames = Arc::new(AtomicUsize::new(0));
        let failed = Arc::new(AtomicBool::new(false));
        let counter = frames.clone();
        let err = failed.clone();
        let render = output.build_output_stream(
            &config.clone().into(),
            move |data: &mut [f32], _| {
                for frame in data.chunks_mut(channels) {
                    let n = counter.fetch_add(1, Ordering::Relaxed);
                    let x = tone(n as f64 / rate as f64);
                    frame.fill(x);
                }
            },
            move |_| {
                err.store(true, Ordering::SeqCst);
            },
            None,
        )?;
        let state = RecordingState::new();
        state.start_recording()?;
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        state.set_audio_sender(tx);
        let mut manager = AudioStreamManager::new(state.clone());
        guarded(&lease)?;
        // This is the product's real Windows system stream path, not injected PCM.
        manager
            .start_streams(
                None,
                Some(Arc::new(AudioDevice::new(name.clone(), DeviceType::Output))),
                None,
            )
            .await?;
        ensure!(
            manager.active_stream_count() == 1 && state.get_microphone_device().is_none(),
            "PC-only stream invariant"
        );
        let mut samples = Vec::new();
        let mut chunks = 0usize;
        let start = Instant::now();
        let mut playing = false;
        while start.elapsed() < Duration::from_millis(6500) {
            guarded(&lease)?;
            ensure!(!failed.load(Ordering::SeqCst), "render stream failed");
            if !playing && start.elapsed() >= Duration::from_millis(600) {
                render.play()?;
                playing = true;
            }
            while let Ok(chunk) = rx.try_recv() {
                ensure!(
                    chunk.device_type == RecordingDeviceType::System && chunk.sample_rate == 48000,
                    "non-PC or wrong-rate capture"
                );
                ensure!(
                    chunk.data.iter().all(|x| x.is_finite()),
                    "nonfinite captured PCM"
                );
                samples.extend(chunk.data);
                chunks += 1;
                ensure!(samples.len() <= 12 * 48000, "capture bound exceeded");
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        manager.stop_streams()?;
        state.stop_recording();
        drop(render);
        while let Ok(chunk) = rx.try_recv() {
            ensure!(
                chunk.device_type == RecordingDeviceType::System && chunk.sample_rate == 48000,
                "bad Stop-tail source"
            );
            samples.extend(chunk.data);
            chunks += 1;
        }
        let after_stop = samples.len();
        tokio::time::sleep(Duration::from_millis(250)).await;
        ensure!(
            rx.try_recv().is_err() && !manager.has_active_streams() && !state.is_recording(),
            "capture continued after Stop"
        );
        ensure!(
            state.get_microphone_device().is_none(),
            "microphone unexpectedly present"
        );
        guarded(&lease)?;
        ensure!(
            state.get_error_count() == 0,
            "capture backend reported errors"
        );
        ensure!(
            samples.len() > 48000 && samples.len() <= 12 * 48000,
            "no/bad captured sample count"
        );
        let peak = samples.iter().fold(0f32, |p, x| p.max(x.abs()));
        if peak < 1e-4 {
            bail!("render played but loopback was silent");
        }
        save_wav(&dir.join("system-captured.wav"), &samples)?;
        let report = serde_json::json!({"stage":"captured-stopped-saved","render_endpoint":name,"render_rate":rate,"render_channels":channels,"render_frames":frames.load(Ordering::SeqCst),"microphone_opened":false,"source":"system","capture_rate":48000,"samples":samples.len(),"chunks":chunks,"peak":peak,"samples_at_stop":after_stop,"streams_after_stop":manager.active_stream_count(),"backend_errors":state.get_error_count(),"real_speech":false,"asr_or_cloud_used":false,"tone_hz":[523.25,739.99,997.0]});
        std::fs::write(
            dir.join("capture-result.json"),
            serde_json::to_vec_pretty(&report)?,
        )?;
        println!("{report}");
        Ok(())
    }
}
#[cfg(windows)]
#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    probe::run().await
}
