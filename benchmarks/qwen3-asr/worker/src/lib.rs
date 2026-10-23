//! Bounded local-file prototype with Silero v4-compatible 512-sample VAD.
use anyhow::{bail, Context, Result};
use serde::Serialize;
use std::{io::BufRead, path::Path};
pub mod alignment;
pub mod vad;

/// Identify the requested weights before ready, without loading tensors.
pub fn model_identity(config: &serde_json::Value) -> anyhow::Result<&'static str> {
    let text = &config["thinker_config"]["text_config"];
    if config["model_type"] == "qwen3_asr" && text["num_hidden_layers"] == 28 {
        match text["hidden_size"].as_u64() {
            Some(2048) => return Ok("Qwen3-ASR-1.7B"),
            Some(1024) => return Ok("Qwen3-ASR-0.6B"),
            _ => {}
        }
    }
    anyhow::bail!("expected_qwen3_asr_0_6b_or_1_7b")
}
#[cfg(test)]
mod model_identity_tests {
    #[test]
    fn exactly_two_model_identities_before_tensor_loading() {
        for (hidden, name) in [(2048, "Qwen3-ASR-1.7B"), (1024, "Qwen3-ASR-0.6B")] {
            assert_eq!(super::model_identity(&serde_json::json!({"model_type":"qwen3_asr","thinker_config":{"text_config":{"hidden_size":hidden,"num_hidden_layers":28}}})).unwrap(),name);
        }
        assert!(super::model_identity(&serde_json::json!({"model_type":"other"})).is_err());
    }
}
pub use vad::speech_intervals;
pub const RATE: usize = 16000;
pub const MAX_SAMPLES: usize = 30 * RATE;
pub const MAX_LINE: usize = 4096;

#[derive(Debug, Clone, Serialize)]
pub struct Interval {
    pub start_sample: usize,
    pub end_sample: usize,
}

/// EOF-tail keeps the real endpoint; padding is solely for the final VAD frame.
pub fn interval(start_ms: usize, end_ms: usize, real_len: usize) -> Option<Interval> {
    let start = start_ms.saturating_mul(16).min(real_len);
    let end = end_ms.saturating_mul(16).min(real_len);
    (end > start).then_some(Interval {
        start_sample: start,
        end_sample: end,
    })
}

pub fn validate_audio(samples: &[f32]) -> Result<()> {
    if samples.is_empty() || samples.len() > MAX_SAMPLES {
        bail!("invalid_audio_duration");
    }
    if samples
        .iter()
        .any(|x| !x.is_finite() || !(-1.0..=1.0).contains(x))
    {
        bail!("invalid_audio_value");
    }
    Ok(())
}

pub fn read_wav(path: &Path) -> Result<Vec<f32>> {
    let mut reader = hound::WavReader::open(path).context("wav_open_failed")?;
    let s = reader.spec();
    let pcm16 = s.bits_per_sample == 16 && s.sample_format == hound::SampleFormat::Int;
    let float32 = s.bits_per_sample == 32 && s.sample_format == hound::SampleFormat::Float;
    if s.channels != 1 || s.sample_rate != RATE as u32 || !(pcm16 || float32) {
        bail!("expected_mono_16k_pcm16_or_float32");
    }
    if reader.duration() == 0 || reader.duration() as usize > MAX_SAMPLES {
        bail!("invalid_audio_duration");
    }
    let mut samples = Vec::with_capacity(reader.duration() as usize);
    if float32 {
        for v in reader.samples::<f32>() {
            if samples.len() >= MAX_SAMPLES {
                bail!("invalid_audio_duration");
            }
            samples.push(v?);
        }
    } else {
        for v in reader.samples::<i16>() {
            if samples.len() >= MAX_SAMPLES {
                bail!("invalid_audio_duration");
            }
            samples.push(v? as f32 / 32768.0);
        }
    }
    validate_audio(&samples)?;
    Ok(samples)
}

/// Bounded gain for VAD only, not an energy-based speech decision. Original PCM
/// remains unchanged for ASR/evidence. Silence stays silence; max gain is 20x.
pub fn vad_gain(samples: &[f32]) -> f32 {
    let peak = samples.iter().fold(0.0f32, |p, x| p.max(x.abs()));
    if peak > 0.0 {
        (0.1 / peak).clamp(1.0, 20.0)
    } else {
        1.0
    }
}
/// Bounded request context: preserve natural internal pauses rather than
/// independently decoding every VAD endpoint (which regressed Thai accuracy).
pub fn recognition_window(
    intervals: &[Interval],
    real_len: usize,
    crop_context: bool,
) -> Vec<Interval> {
    match (intervals.first(), intervals.last()) {
        (Some(first), Some(last)) => vec![Interval {
            start_sample: if crop_context { first.start_sample } else { 0 },
            end_sample: if crop_context {
                last.end_sample
            } else {
                real_len
            },
        }],
        _ => Vec::new(),
    }
}
pub fn merge_intervals(intervals: Vec<Interval>) -> Vec<Interval> {
    let mut merged: Vec<Interval> = Vec::new();
    for i in intervals {
        if let Some(last) = merged.last_mut() {
            if i.start_sample <= last.end_sample {
                last.end_sample = last.end_sample.max(i.end_sample);
                continue;
            }
        }
        merged.push(i);
    }
    merged
}

/// Heuristic review flag, not proof of hallucination. Raw text must be retained.
pub fn review_reasons(text: &str, text_tokens: i32) -> Vec<&'static str> {
    let mut reasons = Vec::new();
    if text.trim().is_empty() {
        reasons.push("empty_recognition");
    }
    // Upstream offline loop has a hard 2048 total-token limit but exposes only
    // text-token count, not EOS. This is a conservative near-limit flag, not an
    // exact EOS assertion. Supervisor timeout is the hard compute bound.
    if text_tokens >= 1900 {
        reasons.push("near_generation_limit");
    }
    if text.chars().count() > 8000 {
        reasons.push("oversized_text");
    }
    let chars: Vec<_> = text.chars().filter(|c| !c.is_whitespace()).collect();
    'search: for period in 1..=32.min(chars.len() / 5) {
        for start in 0..=chars.len() - 5 * period {
            let unit = &chars[start..start + period];
            if period * 5 >= 40
                && (1..5)
                    .all(|rep| &chars[start + rep * period..start + (rep + 1) * period] == unit)
            {
                reasons.push("repetitive_output");
                break 'search;
            }
        }
    }
    reasons
}

/// Fail closed on a too-long line: caller terminates worker, never parses suffix.
pub fn read_line_bounded<R: BufRead>(input: &mut R) -> Result<Option<Vec<u8>>> {
    let mut line = Vec::new();
    loop {
        let available = input.fill_buf()?;
        if available.is_empty() {
            return if line.is_empty() {
                Ok(None)
            } else {
                bail!("truncated_request")
            };
        }
        let take = available
            .iter()
            .position(|&b| b == b'\n')
            .map(|i| i + 1)
            .unwrap_or(available.len());
        if line.len() + take > MAX_LINE {
            bail!("request_too_large");
        }
        let ended = available[take - 1] == b'\n';
        line.extend_from_slice(&available[..take]);
        input.consume(take);
        if ended {
            return Ok(Some(line));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gain_bounded_and_not_silence_gate() {
        assert_eq!(vad_gain(&[0.0]), 1.0);
        assert_eq!(vad_gain(&[0.001]), 20.0);
        assert_eq!(vad_gain(&[0.5]), 1.0);
    }
    #[test]
    fn empty_or_nonfinite_audio_rejected() {
        assert!(validate_audio(&[]).is_err());
        assert!(validate_audio(&[f32::NAN]).is_err());
        assert!(validate_audio(&[1.1]).is_err());
    }
    #[test]
    fn no_speech_tail_has_no_interval() {
        assert!(interval(1000, 1000, 16000).is_none());
    }
    #[test]
    fn last_partial_frame_not_lost() {
        let i = interval(0, 31, 481).unwrap();
        assert_eq!(i.end_sample, 481);
    }
    #[test]
    fn recognition_keeps_internal_pause_and_empty_stays_empty() {
        assert!(recognition_window(&[], 512, false).is_empty());
        let intervals = [
            Interval {
                start_sample: 10,
                end_sample: 100,
            },
            Interval {
                start_sample: 200,
                end_sample: 400,
            },
        ];
        let span = recognition_window(&intervals, 512, false);
        assert_eq!(span.len(), 1);
        assert_eq!(span[0].start_sample, 0);
        assert_eq!(span[0].end_sample, 512);
        let crop = recognition_window(&intervals, 512, true);
        assert_eq!(crop[0].start_sample, 10);
        assert_eq!(crop[0].end_sample, 400);
    }
    #[test]
    fn overlapping_padding_merged() {
        let m = merge_intervals(vec![
            Interval {
                start_sample: 0,
                end_sample: 100,
            },
            Interval {
                start_sample: 90,
                end_sample: 200,
            },
        ]);
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].end_sample, 200);
    }
    #[test]
    fn real_acknowledgements_not_blacklisted() {
        assert!(review_reasons("はい。", 2).is_empty());
        assert!(review_reasons("うん、了解です。", 7).is_empty());
    }
    #[test]
    fn repetition_only_flags_and_limit_flags() {
        assert!(
            review_reasons(&"同じ言葉を繰り返しています。".repeat(8), 64)
                .contains(&"repetitive_output")
        );
        assert!(review_reasons("語尾", 1900).contains(&"near_generation_limit"));
    }
    #[test]
    fn request_limits_and_eof() {
        assert!(read_line_bounded(&mut std::io::Cursor::new(vec![b'x'; MAX_LINE + 1])).is_err());
        assert!(read_line_bounded(&mut std::io::Cursor::new(b"{}")).is_err());
        assert_eq!(
            read_line_bounded(&mut std::io::Cursor::new(b"{}\n"))
                .unwrap()
                .unwrap(),
            b"{}\n"
        );
    }
}
