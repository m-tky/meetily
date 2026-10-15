//! Silero v4-compatible 512-sample forward + sample-based endpointing.
//! Endpoint rules adapted from snakers4/silero-vad v4.0/utils_vad.py
//! (get_speech_timestamps), MIT; see licenses/Silero-MIT.txt.
//! Uses the pinned dependency's embedded v4 weights and h/c state, NOT process(),
//! which internally re-chunks all input into incompatible 480-sample frames.
use crate::{merge_intervals, vad_gain, validate_audio, Interval, RATE};
use anyhow::{bail, Result};
use silero_rs::{VadConfig, VadSession};
pub const FRAME_SAMPLES: usize = 512;
const MIN_SPEECH: usize = RATE * 90 / 1000;
const MIN_SILENCE: usize = RATE / 2;
const PRE: usize = RATE * 300 / 1000;
const POST: usize = RATE * 200 / 1000;

pub fn speech_probabilities(samples: &[f32]) -> Result<Vec<f32>> {
    validate_audio(samples)?;
    // Only forward is called; no stale endpoint/audio buffer/reset behavior.
    let mut model = VadSession::new(VadConfig::default())?;
    let gain = vad_gain(samples);
    let mut probabilities = Vec::with_capacity(samples.len().div_ceil(FRAME_SAMPLES));
    for chunk in samples.chunks(FRAME_SAMPLES) {
        let mut frame: Vec<f32> = chunk.iter().map(|x| x * gain).collect();
        frame.resize(FRAME_SAMPLES, 0.0); // VAD-only; real endpoint retained below.
        let output = model.forward(frame)?;
        let probability = *output
            .try_extract_array::<f32>()?
            .first()
            .ok_or_else(|| anyhow::anyhow!("empty_vad_output"))?;
        if !probability.is_finite() || !(0.0..=1.0).contains(&probability) {
            bail!("invalid_vad_probability");
        }
        probabilities.push(probability);
    }
    Ok(probabilities)
}

/// Faithful v4 trigger/temp-end rules with explicit worker duration/pad settings.
/// No inference on no-speech, energy fallback, blacklist or silence compaction.
pub fn intervals_from_probabilities(
    probabilities: &[f32],
    real_len: usize,
) -> Result<Vec<Interval>> {
    if real_len == 0
        || real_len > crate::MAX_SAMPLES
        || probabilities.len() != real_len.div_ceil(FRAME_SAMPLES)
        || probabilities
            .iter()
            .any(|p| !p.is_finite() || !(0.0..=1.0).contains(p))
    {
        bail!("invalid_vad_frames");
    }
    let mut start = None;
    let mut temp_end = None;
    let mut raw = Vec::new();
    for (index, &probability) in probabilities.iter().enumerate() {
        let current = index * FRAME_SAMPLES;
        if probability >= 0.5 {
            temp_end = None;
            if start.is_none() {
                start = Some(current);
                continue;
            }
        }
        if probability < 0.35 {
            if let Some(begin) = start {
                let end = *temp_end.get_or_insert(current);
                if current - end < MIN_SILENCE {
                    continue;
                }
                if end - begin > MIN_SPEECH {
                    raw.push((begin, end));
                }
                temp_end = None;
                start = None;
            }
        }
    }
    // v4 EOF policy includes real tail while active, never padded sample count.
    if let Some(begin) = start {
        if real_len - begin > MIN_SPEECH {
            raw.push((begin, real_len));
        }
    }
    Ok(merge_intervals(
        raw.into_iter()
            .map(|(begin, end)| Interval {
                start_sample: begin.saturating_sub(PRE),
                end_sample: end.saturating_add(POST).min(real_len),
            })
            .collect(),
    ))
}

pub fn speech_intervals(samples: &[f32]) -> Result<Vec<Interval>> {
    intervals_from_probabilities(&speech_probabilities(samples)?, samples.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn official_geometry_and_short_speech_at_eof() {
        assert_eq!(FRAME_SAMPLES, 512);
        assert!(intervals_from_probabilities(&[0.0; 16], 8000)
            .unwrap()
            .is_empty());
        let p = vec![0.9; 4];
        let v = intervals_from_probabilities(&p, 1537).unwrap();
        assert_eq!(v[0].end_sample, 1537);
        assert!(intervals_from_probabilities(&[0.9; 3], 1440)
            .unwrap()
            .is_empty());
    }
    #[test]
    fn padding_does_not_authorize_under_minimum_real_speech() {
        assert!(intervals_from_probabilities(&[0.9], 1).unwrap().is_empty());
    }
    #[test]
    fn endpoint_hysteresis_and_real_tail() {
        let mut p = vec![0.9; 5];
        p.extend(vec![0.0; 18]);
        let v = intervals_from_probabilities(&p, p.len() * 512).unwrap();
        assert_eq!(v[0].end_sample, 5 * 512 + POST);
        // Recovery from candidate silence cancels temp_end, not the utterance.
        let mut p = vec![0.9; 5];
        p.extend(vec![0.0; 5]);
        p.push(0.9);
        let v = intervals_from_probabilities(&p, p.len() * 512 - 1).unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].end_sample, p.len() * 512 - 1);
    }
    #[test]
    fn invalid_probabilities_and_frame_counts_reject() {
        assert!(intervals_from_probabilities(&[f32::NAN], 512).is_err());
        assert!(intervals_from_probabilities(&[1.1], 512).is_err());
        assert!(intervals_from_probabilities(&[], 512).is_err());
    }
}
