//! Research only: same embedded v4 model, compare neural frame geometry.
//! No ASR, no endpoint-policy replacement, no capture/application integration.
use anyhow::{bail, Result};
use qwen_speech_worker::{read_wav, vad_gain};
use serde_json::json;
use silero_rs::{VadConfig, VadSession};
fn main() -> Result<()> {
    ort::init_from(std::env::var("ORT_DYLIB_PATH")?).commit()?;
    for path in std::env::args().skip(1) {
        let audio = read_wav(std::path::Path::new(&path))?;
        for frame_size in [480, 512] {
            for gain in [1.0, vad_gain(&audio)] {
                let mut model = VadSession::new(VadConfig::default())?;
                let mut probabilities = Vec::new();
                for chunk in audio.chunks(frame_size) {
                    let mut frame: Vec<f32> = chunk.iter().map(|x| x * gain).collect();
                    frame.resize(frame_size, 0.0);
                    let output = model.forward(frame)?;
                    let probability = *output.try_extract_array::<f32>()?.first().unwrap();
                    if !probability.is_finite() {
                        bail!("nonfinite_vad");
                    }
                    probabilities.push(probability);
                }
                println!(
                    "{}",
                    json!({"file":std::path::Path::new(&path).file_name().unwrap().to_str(),"frame_size":frame_size,"gain":gain,"real_samples":audio.len(),"max_probability":probabilities.iter().copied().fold(0.0f32,f32::max),"positive_frames":probabilities.iter().filter(|&&p|p>0.5).count(),"frame_probabilities":probabilities})
                );
            }
        }
    }
    Ok(())
}
