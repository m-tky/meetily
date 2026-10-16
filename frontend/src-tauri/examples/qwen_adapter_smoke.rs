//! App adapter smoke: generated silence and optionally an explicitly supplied
//! public mono16k PCM16/float32 WAV. No device capture, playback or cloud access.
use app_lib::audio::transcription::{
    qwen_provider::{QwenProvider, QwenSettings},
    TranscriptionProvider,
};

fn public_wav(path: &str) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
    let bytes = std::fs::read(path)?;
    if bytes.len() < 12 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("Expected RIFF WAV".into());
    }
    let mut offset = 12;
    let mut format = None;
    let mut payload = None;
    while offset + 8 <= bytes.len() {
        let size = u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into()?) as usize;
        let start = offset + 8;
        let end = start.checked_add(size).ok_or("Invalid WAV size")?;
        let data = bytes.get(start..end).ok_or("Truncated WAV")?;
        match &bytes[offset..offset + 4] {
            b"fmt " if size >= 16 => {
                if data[2..4] != [1, 0] || data[4..8] != 16000u32.to_le_bytes() {
                    return Err("Expected mono16k WAV".into());
                }
                format = Some((
                    u16::from_le_bytes(data[..2].try_into()?),
                    u16::from_le_bytes(data[14..16].try_into()?),
                ));
            }
            b"data" => {
                payload = Some(data);
            }
            _ => {}
        }
        offset = end.checked_add(size % 2).ok_or("Invalid WAV padding")?;
    }
    let data = payload.ok_or("WAV data missing")?;
    match format {
        Some((3, 32)) if data.len() % 4 == 0 => Ok(data
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
            .collect()),
        // Exactly the same lossless PCM16 scaling as the real worker reader.
        Some((1, 16)) if data.len() % 2 == 0 => Ok(data
            .chunks_exact(2)
            .map(|b| i16::from_le_bytes(b.try_into().unwrap()) as f32 / 32768.0)
            .collect()),
        _ => Err("Expected mono16k PCM16/IEEE float32 WAV".into()),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if !(4..=5).contains(&args.len()) {
        return Err("usage: qwen_adapter_smoke WORKER MODEL_DIR ORT_LIBRARY [PUBLIC_WAV]".into());
    }
    let cache = tempfile::tempdir()?;
    let audio_dir = cache.path().join("日本語 O'Brien 空白");
    let provider = QwenProvider::new(
        QwenSettings {
            worker_path: args[1].clone().into(),
            model_dir: args[2].clone().into(),
            ort_library: args[3].clone().into(),
        },
        audio_dir.clone(),
    )?;
    provider.connect().await?;
    for code in ["ja", "en", "th"] {
        let result = provider
            .transcribe(vec![0.0; 5 * 16000], Some(code.into()))
            .await?;
        if !result.text.is_empty() {
            return Err("Synthetic silence unexpectedly produced text".into());
        }
    }
    if let Some(wav) = args.get(4) {
        let audio = public_wav(wav)?;
        let start = std::time::Instant::now();
        let first = provider
            .transcribe(audio.clone(), Some("ja".into()))
            .await?;
        if first.text.is_empty() {
            return Err("Public speech produced no transcript".into());
        }
        println!(
            "{}",
            serde_json::json!({"event":"public_speech","text":first.text,"wall_seconds":start.elapsed().as_secs_f64()})
        );
        let second = provider.transcribe(audio, Some("ja".into())).await?;
        if first.text != second.text {
            return Err("Repeated public speech text differed".into());
        }
    }
    provider.shutdown().await;
    if std::fs::read_dir(&audio_dir)?.count() != 0 {
        return Err("Temporary audio was retained".into());
    }
    println!("App Qwen adapter: handshake, float32 silence, optional public speech/repeat, explicit worker shutdown and Unicode-path temporary-file cleanup passed. No recording / cloud / general quality claim.");
    Ok(())
}
