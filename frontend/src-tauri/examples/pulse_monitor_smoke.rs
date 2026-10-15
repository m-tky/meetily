//! Synthetic null-sink only. Never opens microphone/default desktop capture.
//! App AudioStream -> AudioCapture -> RecordingState channel; no ASR/cloud.
#[cfg(target_os = "linux")]
mod linux {
    use anyhow::{ensure, Result};
    use app_lib::audio::{
        capture::pulse::{monitor_names, PulseCapture, PREFIX},
        devices::{AudioDevice, DeviceType as DeviceKind},
        recording_state::{DeviceType, RecordingState},
        stream::AudioStream,
    };
    use std::{
        io::Write,
        process::Command,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
        time::{Duration, Instant},
    };
    struct Sink {
        id: String,
        name: String,
    }
    impl Sink {
        fn create(name: String) -> Result<Self> {
            let output = Command::new("pactl")
                .args([
                    "load-module",
                    "module-null-sink",
                    &format!("sink_name={name}"),
                    "rate=48000",
                    "channels=1",
                    "sink_properties=device.description=MeetilySyntheticSmoke",
                ])
                .output()?;
            ensure!(output.status.success(), "Cannot create dedicated null sink");
            let id = String::from_utf8(output.stdout)?.trim().to_owned();
            ensure!(id.parse::<u32>().is_ok(), "Invalid module ID");
            Ok(Self { id, name })
        }
        fn device(&self) -> String {
            format!("{PREFIX}{}.monitor", self.name)
        }
    }
    impl Drop for Sink {
        fn drop(&mut self) {
            let _ = Command::new("pactl")
                .args(["unload-module", &self.id])
                .output();
        }
    }
    async fn capture_tone(sink: &Sink, tone: &std::path::Path) -> Result<()> {
        let state = RecordingState::new();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        state.set_audio_sender(tx);
        state.start_recording()?;
        let device = Arc::new(AudioDevice::new(sink.device(), DeviceKind::Output));
        let stream = AudioStream::create(device, state.clone(), DeviceType::System, None).await?;
        let mut playback = tokio::process::Command::new("paplay")
            .args([
                "--raw",
                "--format=float32le",
                "--rate=48000",
                "--channels=1",
                "--device",
                &sink.name,
            ])
            .arg(tone)
            .kill_on_drop(true)
            .spawn()?;
        let start = Instant::now();
        let mut samples = 0;
        let mut active_samples = 0;
        while start.elapsed() < Duration::from_secs(5) {
            if let Ok(Some(chunk)) =
                tokio::time::timeout(Duration::from_millis(100), rx.recv()).await
            {
                ensure!(
                    chunk.sample_rate == 48000 && chunk.device_type == DeviceType::System,
                    "Incorrect app chunk metadata"
                );
                ensure!(
                    chunk.data.iter().all(|v| v.is_finite()),
                    "Nonfinite app samples"
                );
                samples += chunk.data.len();
                active_samples += chunk.data.iter().filter(|v| v.abs() > 0.02).count();
            }
            if playback.try_wait()?.is_some() && active_samples > 48000 {
                break;
            }
        }
        let _ = playback.kill().await;
        state.pause_recording()?;
        // Drain queued pre-pause data, then verify callbacks stop producing chunks.
        tokio::time::sleep(Duration::from_millis(100)).await;
        while rx.try_recv().is_ok() {}
        tokio::time::sleep(Duration::from_millis(100)).await;
        ensure!(rx.try_recv().is_err(), "Pause still produced app chunks");
        state.resume_recording()?;
        let stop = Instant::now();
        stream.stop()?;
        ensure!(
            stop.elapsed() < Duration::from_secs(1),
            "Pulse stop blocked"
        );
        state.stop_recording();
        ensure!(
            samples >= 48000 && active_samples >= 48000,
            "Synthetic tone did not reach the app pipeline"
        );
        println!("Synthetic app capture: {samples} samples, {active_samples} tone samples, pause/resume/stop passed");
        Ok(())
    }
    pub async fn run() -> Result<()> {
        // Introspection only: do not capture the actual default monitor.
        let default = app_lib::audio::capture::pulse::default_monitor_name()?;
        ensure!(
            monitor_names()?.contains(&default),
            "Default Pulse monitor resolution is inconsistent"
        );
        let root = tempfile::tempdir()?;
        let tone = root.path().join("synthetic.f32");
        let mut file = std::fs::File::create(&tone)?;
        for i in 0..48000 * 3 {
            file.write_all(
                &(0.1f32 * (std::f32::consts::TAU * 440.0 * i as f32 / 48000.0).sin())
                    .to_le_bytes(),
            )?;
        }
        drop(file);
        let name = format!("meetily_synthetic_{}", uuid::Uuid::new_v4().simple());
        let sink = Sink::create(name.clone())?;
        ensure!(
            monitor_names()?.contains(&sink.device()),
            "Native discovery omitted dedicated monitor"
        );
        capture_tone(&sink, &tone).await?;
        capture_tone(&sink, &tone).await?; // repeated start/stop on same source
        let disconnected = Arc::new(AtomicBool::new(false));
        let flag = disconnected.clone();
        let device = sink.device();
        let capture = PulseCapture::start(
            &device,
            |_| {},
            move |_| {
                flag.store(true, Ordering::Release);
            },
        )?;
        drop(sink);
        let start = Instant::now();
        while !disconnected.load(Ordering::Acquire) && start.elapsed() < Duration::from_secs(3) {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        ensure!(
            disconnected.load(Ordering::Acquire),
            "Source removal did not report an error"
        );
        drop(capture);
        ensure!(
            PulseCapture::start(&device, |_| {}, |_| {}).is_err(),
            "Missing source was silently replaced"
        );
        // Explicitly recreate the same dedicated source and reconnect.
        let sink = Sink::create(name)?;
        capture_tone(&sink, &tone).await?;
        drop(sink);
        ensure!(
            !monitor_names()?.contains(&device),
            "Synthetic module cleanup failed"
        );
        println!("Monitor removal, no fallback, explicit reconnect, module cleanup passed. No microphone / real desktop / ASR / cloud.");
        Ok(())
    }
}
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    #[cfg(target_os = "linux")]
    {
        linux::run().await
    }
    #[cfg(not(target_os = "linux"))]
    {
        anyhow::bail!("Linux only")
    }
}
