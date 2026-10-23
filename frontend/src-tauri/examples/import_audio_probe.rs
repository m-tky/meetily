//! Offline diagnostic of the production decoder/resampler/batch VAD.
//! Explicit input/output paths only; no capture, playback, inference or app profile.
//! Developer tests use licensed public fixtures, never private meeting audio.
use app_lib::audio::{decoder::decode_audio_file, vad::get_speech_chunks};
use std::{fs, io::Write, path::Path};

fn wav(path: &Path, samples: &[f32], rate: u32, channels: u16) -> anyhow::Result<()> {
    let bytes = u32::try_from(samples.len() * 4)?;
    let file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    let mut f = std::io::BufWriter::new(file);
    f.write_all(b"RIFF")?;
    f.write_all(&(36 + bytes).to_le_bytes())?;
    f.write_all(b"WAVEfmt ")?;
    f.write_all(&16u32.to_le_bytes())?;
    f.write_all(&3u16.to_le_bytes())?;
    f.write_all(&channels.to_le_bytes())?;
    f.write_all(&rate.to_le_bytes())?;
    f.write_all(&(rate * channels as u32 * 4).to_le_bytes())?;
    f.write_all(&(channels * 4).to_le_bytes())?;
    f.write_all(&32u16.to_le_bytes())?;
    f.write_all(b"data")?;
    f.write_all(&bytes.to_le_bytes())?;
    for s in samples {
        f.write_all(&s.to_le_bytes())?;
    }
    f.flush()?;
    Ok(())
}
fn main() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    anyhow::ensure!(
        args.len() == 3,
        "usage: import_audio_probe EXPLICIT_PUBLIC_INPUT NEW_OUTPUT_DIRECTORY"
    );
    let input = Path::new(&args[1]);
    let out = Path::new(&args[2]);
    fs::create_dir(out)?;
    let audio = decode_audio_file(input)?;
    wav(
        &out.join("decoded.wav"),
        &audio.samples,
        audio.sample_rate,
        audio.channels,
    )?;
    let pcm = audio.to_whisper_format();
    wav(&out.join("pre-vad.wav"), &pcm, 16000, 1)?;
    let segments = get_speech_chunks(&pcm, 2000)?;
    let mut rows = Vec::new();
    for (i, segment) in segments.iter().enumerate() {
        let filename = format!("segment-{i}.wav");
        wav(&out.join(&filename), &segment.samples, 16000, 1)?;
        rows.push(serde_json::json!({"file":filename,"samples":segment.samples.len(),"start_ms":segment.start_timestamp_ms,"end_ms":segment.end_timestamp_ms}));
    }
    fs::write(
        out.join("probe.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "sample_rate":audio.sample_rate,"channels":audio.channels,"decoded_samples":audio.samples.len(),
            "duration_seconds":audio.duration_seconds,"pre_vad_samples":pcm.len(),"segments":rows
        }))?,
    )?;
    Ok(())
}
