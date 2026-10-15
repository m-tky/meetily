//! Opt-in character alignment of unchanged ASR/provided text, not recognition verification.
use anyhow::{bail, Result};
use qwen_asr::{align, context::QwenModel};
use serde_json::{json, Value};
use std::{sync::Arc, time::Instant};

pub const MAX_ALIGN_CHARS: usize = 512;

pub fn validate_alignment_text(text: &str, language: &str) -> Result<()> {
    // Bounded Japanese experiment only; do not claim parity with official nagisa units.
    if language != "Japanese" {
        bail!("alignment_language_not_supported");
    }
    let count = text.chars().filter(|c| !c.is_whitespace()).count();
    if count == 0 || count > MAX_ALIGN_CHARS {
        bail!("invalid_alignment_text_length");
    }
    Ok(())
}

pub fn align_text(
    model_dir: &str,
    model: &mut Option<Arc<QwenModel>>,
    samples: &[f32],
    text: &str,
    language: &str,
) -> Result<Value> {
    validate_alignment_text(text, language)?;
    let loaded = Instant::now();
    let mut load_seconds = 0.0;
    if model.is_none() {
        let m = QwenModel::load(model_dir)
            .ok_or_else(|| anyhow::anyhow!("aligner_model_load_failed"))?;
        if !m.config.is_aligner() {
            bail!("not_a_forced_aligner_model");
        }
        *model = Some(m);
        load_seconds = loaded.elapsed().as_secs_f64();
    }
    let mut ctx = model.as_ref().unwrap().new_session();
    let tick = Instant::now();
    let units = align::forced_align(&mut ctx, samples, text, language)
        .ok_or_else(|| anyhow::anyhow!("alignment_failed"))?;
    let elapsed = tick.elapsed().as_secs_f64();
    let duration = samples.len() as f64 / 16000.0;
    let input_chars: Vec<_> = text
        .char_indices()
        .filter(|(_, c)| !c.is_whitespace())
        .collect();
    let mut warnings = Vec::new();
    let expected: String = input_chars.iter().map(|(_, c)| *c).collect();
    let actual: String = units.iter().map(|u| u.text.as_str()).collect();
    if actual != expected || units.len() != input_chars.len() {
        warnings.push("text_unit_mismatch");
    }
    let mut previous_end = 0.0;
    let mut output = Vec::new();
    for (i, u) in units.iter().enumerate() {
        let a = u.start_ms as f64 / 1000.0;
        let b = u.end_ms as f64 / 1000.0;
        let mut flags = Vec::new();
        if !a.is_finite() || !b.is_finite() {
            flags.push("nonfinite_time");
        } else {
            if a < 0.0 || b < a || b > duration + 0.000001 {
                flags.push("out_of_bounds_time");
            }
            if a < previous_end - 0.000001 {
                flags.push("nonmonotonic_time");
            }
            if b <= a {
                flags.push("zero_duration");
            }
            previous_end = b;
        }
        if u.text.chars().all(|c| !c.is_alphanumeric()) {
            flags.push("non_spoken_symbol");
        }
        let offset = input_chars
            .get(i)
            .map(|(byte, c)| (*byte, *byte + c.len_utf8()));
        output.push(
            json!({"text":u.text,"start_seconds":if a.is_finite(){Some(a)}else{None},
            "end_seconds":if b.is_finite(){Some(b)}else{None},
            "text_start_byte":offset.map(|x|x.0),"text_end_byte":offset.map(|x|x.1),
            "flags":flags,"confirmed":false}),
        );
    }
    Ok(json!({"status":"aligned","unit":"unicode_scalar",
        "implementation":"rust_character_units_not_official_nagisa_word_units",
        "timestamps_postprocessed_by_upstream_lis_interpolation":true,
        "input_samples":samples.len(),"text":text,"units":output,"warnings":warnings,
        "load_seconds":load_seconds,"inference_seconds":elapsed,
        "confirmed":false,"recognition_verified":false}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn alignment_is_bounded_and_japanese_only() {
        assert!(validate_alignment_text("はい。", "Japanese").is_ok());
        assert!(validate_alignment_text("  \n", "Japanese").is_err());
        assert!(validate_alignment_text(&"本".repeat(513), "Japanese").is_err());
        assert!(validate_alignment_text("本", "Thai").is_err());
    }
}
