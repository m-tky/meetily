//! Offline fixture evaluation through the SAME WhisperEngine/WhisperProvider as Meetily.
//! No app startup, microphone, playback, telemetry, network or credentials.
//! cargo run --locked -p meetily --example asr_fixture -- <artifact-dir> base|small
//! --devices only lists CPAL devices; it does not open capture streams.
use anyhow::{bail, Context, Result};
use app_lib::audio::transcription::{
    provider::TranscriptionProvider, whisper_provider::WhisperProvider,
};
use app_lib::whisper_engine::WhisperEngine;
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{fs, io::Read, path::Path, sync::Arc, time::Instant};

#[derive(Deserialize)]
struct Fixture {
    rows: Vec<Row>,
}
#[derive(Deserialize)]
struct Row {
    id: String,
    reference: String,
    audio: String,
    samples: usize,
    pcm_sha256: String,
}
#[derive(Deserialize)]
struct Model {
    name: String,
    sha256: String,
}

fn hash(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        digest.update(&buffer[..n]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--devices") {
        use cpal::traits::{DeviceTrait, HostTrait};
        let host = cpal::default_host();
        let inputs: Vec<_> = host
            .input_devices()?
            .map(|d| d.name())
            .collect::<std::result::Result<_, _>>()?;
        let outputs: Vec<_> = host
            .output_devices()?
            .map(|d| d.name())
            .collect::<std::result::Result<_, _>>()?;
        println!(
            "{}",
            json!({"host": format!("{:?}", host.id()), "inputs": inputs, "outputs": outputs})
        );
        return Ok(());
    }
    if args.len() != 3 || !["base", "small"].contains(&args[2].as_str()) {
        bail!("Usage: asr_fixture <artifact-directory> base|small, or --devices");
    }
    let root = Path::new(&args[1]).canonicalize()?;
    let models_dir = root.join("models");
    let models: Vec<Model> = serde_json::from_slice(&fs::read(root.join("models.json"))?)?;
    let model = models
        .iter()
        .find(|m| m.name == args[2])
        .context("Model metadata missing")?;
    let model_sha = hash(&models_dir.join(format!("ggml-{}.bin", model.name)))?;
    if model_sha != model.sha256 {
        bail!("Model hash mismatch");
    }
    let fixture: Fixture = serde_json::from_slice(&fs::read(root.join("manifest.json"))?)?;
    if fixture.rows.is_empty() || fixture.rows.len() > 100 {
        bail!("Invalid fixture count");
    }
    let engine = Arc::new(WhisperEngine::new_with_models_dir(Some(models_dir))?);
    engine.discover_models().await?;
    let start = Instant::now();
    engine.load_model(&model.name).await?;
    let load_seconds = start.elapsed().as_secs_f64();
    let provider = WhisperProvider::new(engine);
    for row in fixture.rows {
        let path = root.join(&row.audio).canonicalize()?;
        if !path.starts_with(&root) || row.samples == 0 || row.samples > 60 * 16000 {
            bail!("Invalid fixture path or sample count");
        }
        if hash(&path)? != row.pcm_sha256 {
            bail!("PCM hash mismatch");
        }
        let bytes = fs::read(&path)?;
        if bytes.len() != row.samples * 4 {
            bail!("PCM sample count mismatch");
        }
        let samples: Vec<f32> = bytes
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
            .collect();
        if samples.iter().any(|v| !v.is_finite() || v.abs() > 1.0) {
            bail!("Invalid PCM samples");
        }
        let start = Instant::now();
        let result = provider.transcribe(samples, Some("ja".into())).await?;
        let elapsed_seconds = start.elapsed().as_secs_f64();
        println!(
            "{}",
            json!({"id": row.id, "reference": row.reference, "text": result.text,
            "audio_seconds": row.samples as f64 / 16000.0, "elapsed_seconds": elapsed_seconds,
            "is_partial": result.is_partial, "model": model.name, "model_sha256": model_sha,
            "load_seconds": load_seconds, "language": "ja", "build": if cfg!(debug_assertions) {"debug"} else {"release"},
            "meetily_version": env!("CARGO_PKG_VERSION")})
        );
    }
    Ok(())
}
