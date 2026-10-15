//! Bounded, per-PC-source loudness control before mixing (mono 48k capture).
//! No lookahead/delay/tail buffer: every input sample produces one output sample.
//! Sample-peak protection is NOT a reconstructed true-peak guarantee.
use anyhow::Result;

const TARGET_LUFS: f64 = -23.0;
const MAX_GAIN_DB: f64 = 20.0;
const MIN_GAIN_DB: f64 = -40.0;
const QUIET_RMS: f64 = 0.0001; // -80dBFS: do not chase silence/very low noise.
const SAMPLE_PEAK_LIMIT: f32 = 0.8912509; // -1dBFS

pub(crate) struct PcLoudnessNormalizer {
    meter: ebur128::EbuR128,
    analysis: Vec<f32>,
    block_samples: usize,
    energy: f64,
    gain: f32,
    target_gain: f32,
    smoothing: f32,
}

impl PcLoudnessNormalizer {
    pub(crate) fn new(sample_rate: u32) -> Result<Self> {
        let block_samples = (sample_rate as usize / 10).max(1);
        Ok(Self {
            meter: ebur128::EbuR128::new(1, sample_rate, ebur128::Mode::M)?,
            analysis: Vec::with_capacity(block_samples),
            block_samples,
            energy: 0.0,
            gain: 1.0,
            target_gain: 1.0,
            // 300ms exponential smoothing, invariant to callback chunk size.
            smoothing: (1.0 - (-1.0 / (sample_rate as f64 * 0.3)).exp()) as f32,
        })
    }

    pub(crate) fn process(&mut self, samples: &[f32]) -> Vec<f32> {
        let mut output = Vec::with_capacity(samples.len());
        for &input in samples {
            let sample = if input.is_finite() { input } else { 0.0 };
            self.analysis.push(sample);
            self.energy += (sample as f64).powi(2);
            if self.analysis.len() == self.block_samples {
                let rms = (self.energy / self.block_samples as f64).sqrt();
                let measured = self
                    .meter
                    .add_frames_f32(&self.analysis)
                    .and_then(|_| self.meter.loudness_momentary());
                self.target_gain = match measured {
                    Ok(lufs) if rms > QUIET_RMS && lufs.is_finite() => {
                        let db = (TARGET_LUFS - lufs).clamp(MIN_GAIN_DB, MAX_GAIN_DB);
                        10.0_f64.powf(db / 20.0) as f32
                    }
                    _ => 1.0,
                };
                self.analysis.clear();
                self.energy = 0.0;
            }
            self.gain += self.smoothing * (self.target_gain - self.gain);
            // Bound every sample, including sudden transients before the next
            // loudness estimate. Never silently shift samples or invent a tail.
            output.push((sample * self.gain).clamp(-SAMPLE_PEAK_LIMIT, SAMPLE_PEAK_LIMIT));
        }
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tone(seconds: usize, amplitude: f32) -> Vec<f32> {
        (0..seconds * 48000)
            .map(|i| amplitude * (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 48000.0).sin())
            .collect()
    }
    fn rms(x: &[f32]) -> f64 {
        (x.iter().map(|x| (*x as f64).powi(2)).sum::<f64>() / x.len() as f64).sqrt()
    }
    #[test]
    fn silence_and_empty_remain_exact() {
        let mut n = PcLoudnessNormalizer::new(48000).unwrap();
        assert!(n.process(&[]).is_empty());
        assert_eq!(n.process(&vec![0.0; 96000]), vec![0.0; 96000]);
        assert_eq!(n.gain, 1.0);
    }
    #[test]
    fn quiet_tone_boost_is_capped_at_20db() {
        let mut n = PcLoudnessNormalizer::new(48000).unwrap();
        let x = tone(4, 0.002);
        let y = n.process(&x);
        let ratio = rms(&y[144000..]) / rms(&x[144000..]);
        assert!(ratio > 9.9 && ratio <= 10.001, "gain={ratio}");
        assert!(n.target_gain <= 10.0 && n.gain <= 10.0);
    }
    #[test]
    fn normal_level_converges_to_target_lufs() {
        let mut n = PcLoudnessNormalizer::new(48000).unwrap();
        let y = n.process(&tone(6, 0.03));
        let mut meter = ebur128::EbuR128::new(1, 48000, ebur128::Mode::I).unwrap();
        meter.add_frames_f32(&y[192000..]).unwrap();
        let lufs = meter.loudness_global().unwrap();
        assert!((lufs - TARGET_LUFS).abs() < 0.5, "lufs={lufs}");
    }
    #[test]
    fn loud_tone_is_attenuated_and_peak_bounded() {
        let mut n = PcLoudnessNormalizer::new(48000).unwrap();
        let x = tone(4, 0.8);
        let y = n.process(&x);
        assert!(rms(&y[144000..]) < rms(&x[144000..]) * 0.25);
        assert!(y.iter().all(|x| x.abs() <= SAMPLE_PEAK_LIMIT));
    }
    #[test]
    fn sudden_overrange_samples_are_bounded_without_delay() {
        let mut n = PcLoudnessNormalizer::new(48000).unwrap();
        let y = n.process(&[0.0, 2.0, -2.0, 0.0]);
        assert_eq!(y, vec![0.0, SAMPLE_PEAK_LIMIT, -SAMPLE_PEAK_LIMIT, 0.0]);
    }
    #[test]
    fn low_noise_does_not_trigger_upward_gain() {
        let mut n = PcLoudnessNormalizer::new(48000).unwrap();
        let x = vec![0.000001; 96000];
        assert_eq!(n.process(&x), x);
        assert_eq!(n.target_gain, 1.0);
    }
    #[test]
    fn callback_chunking_is_bit_exact() {
        let x = tone(2, 0.02);
        let mut a = PcLoudnessNormalizer::new(48000).unwrap();
        let mut b = PcLoudnessNormalizer::new(48000).unwrap();
        let expected = a.process(&x);
        let mut actual = Vec::new();
        for chunk in x.chunks(997) {
            actual.extend(b.process(chunk));
        }
        assert_eq!(actual, expected);
    }
    #[test]
    fn nonfinite_input_does_not_poison_state() {
        let mut n = PcLoudnessNormalizer::new(48000).unwrap();
        assert_eq!(
            n.process(&[f32::NAN, f32::INFINITY, f32::NEG_INFINITY]),
            vec![0.0; 3]
        );
        assert!(n.process(&tone(1, 0.03)).iter().all(|x| x.is_finite()));
    }
    #[test]
    fn input_length_and_silence_after_signal_are_preserved() {
        let mut n = PcLoudnessNormalizer::new(48000).unwrap();
        let x = tone(1, 0.002);
        assert_eq!(n.process(&x).len(), x.len());
        assert_eq!(n.process(&vec![0.0; 24001]), vec![0.0; 24001]);
    }
    #[test]
    fn gain_changes_are_smoothed_per_sample() {
        let mut n = PcLoudnessNormalizer::new(48000).unwrap();
        n.process(&tone(1, 0.002));
        let gain = n.gain;
        n.process(&[0.002]);
        assert!((n.gain - gain).abs() < 0.001);
    }
}
