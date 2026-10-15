//! Pure reference-test bridge. No ONNX/model inference.
use anyhow::Result;
use serde::Deserialize;
use std::io::{self, BufRead};
#[derive(Deserialize)]
struct Case {
    probabilities: Vec<f32>,
    real_samples: usize,
}
fn main() -> Result<()> {
    for line in io::stdin().lock().lines() {
        let c: Case = serde_json::from_str(&line?)?;
        let intervals = qwen_speech_worker::vad::intervals_from_probabilities(
            &c.probabilities,
            c.real_samples,
        )?;
        println!("{}", serde_json::to_string(&intervals)?);
    }
    Ok(())
}
