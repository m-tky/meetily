//! Diagnostic gain ablation only. Same pinned v4 weights and 512-sample forward.
//! Does not alter worker gating, ASR PCM, dependencies or app policy.
use anyhow::{bail, Result};
use qwen_speech_worker::{read_wav, vad::intervals_from_probabilities, vad_gain};
use serde_json::json;
use silero_rs::{VadConfig, VadSession};

fn main() -> Result<()> {
    ort::init_from(std::env::var("ORT_DYLIB_PATH")?).commit()?;
    for path in std::env::args().skip(1) {
        let samples = read_wav(std::path::Path::new(&path))?;
        let peak = samples.iter().fold(0.0f32, |p, x| p.max(x.abs()));
        let peak100 = if peak > 0.0 {
            (0.1 / peak).clamp(1.0, 100.0)
        } else {
            1.0
        };
        for (policy, gain) in [
            ("none", 1.0),
            ("peak20", vad_gain(&samples)),
            ("peak100", peak100),
        ] {
            let mut session = VadSession::new(VadConfig::default())?;
            let mut probabilities = Vec::new();
            for chunk in samples.chunks(512) {
                let mut frame: Vec<f32> = chunk.iter().map(|x| x * gain).collect();
                frame.resize(512, 0.0);
                let output = session.forward(frame)?;
                let p = *output
                    .try_extract_array::<f32>()?
                    .first()
                    .ok_or_else(|| anyhow::anyhow!("empty_vad_output"))?;
                if !p.is_finite() || !(0.0..=1.0).contains(&p) {
                    bail!("invalid_vad_probability");
                }
                probabilities.push(p);
            }
            println!(
                "{}",
                json!({"file":path,"policy":policy,"gain":gain,
                "real_samples":samples.len(),"frame_samples":512,
                "default_intervals":intervals_from_probabilities(&probabilities,samples.len())?,
                "probabilities":probabilities})
            );
        }
    }
    Ok(())
}
