//! Map a continuous backend sample clock to the session clock. Backend timing
//! snapshots are noisy; only the initial valid capture timestamp anchors a run.
//! Subsequent valid snapshots validate the clock (100ms maximum discrepancy),
//! not repeatedly insert/remove samples. A larger VALID backend discontinuity
//! starts a new run at that time: the mixer retains/reports the gap or overlap.
//! Missing timestamps remain unknown; pause-resume starts a new run.
#[derive(Default)]
pub(crate) struct CaptureClock {
    anchor: Option<f64>,
    frames: u64,
    generation: u64,
}
impl CaptureClock {
    pub fn stamp(
        &mut self,
        measured: Option<f64>,
        frames: usize,
        rate: u32,
        generation: u64,
    ) -> Option<f64> {
        if self.generation != generation {
            self.reset();
            self.generation = generation;
        }
        let count = self.frames;
        self.frames = self.frames.saturating_add(frames as u64);
        let measured = measured.filter(|v| v.is_finite())?;
        if rate == 0 {
            return None;
        }
        let offset = count as f64 / rate as f64;
        let anchor = *self.anchor.get_or_insert(measured - offset);
        let expected = anchor + offset;
        if (expected - measured).abs() > 0.100 {
            // Do not leave a returning backend permanently unknown, or hide
            // its missing interval by continuing the old sample clock.
            self.anchor = Some(measured);
            self.frames = frames as u64;
            return Some(measured);
        }
        Some(expected)
    }
    pub fn reset(&mut self) {
        self.anchor = None;
        self.frames = 0;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn starts_at_backend_time_not_arrival_or_zero() {
        let mut c = CaptureClock::default();
        assert_eq!(c.stamp(Some(1.85), 480, 48000, 0), Some(1.85));
        assert_eq!(c.stamp(Some(1.861), 480, 48000, 0), Some(1.86));
    }
    #[test]
    fn scheduling_noise_does_not_reposition_contiguous_samples() {
        let mut c = CaptureClock::default();
        c.stamp(Some(0.5), 480, 48000, 0);
        assert_eq!(c.stamp(Some(0.540), 480, 48000, 0), Some(0.51));
    }
    #[test]
    fn missing_timing_does_not_fallback_but_keeps_sample_position() {
        let mut c = CaptureClock::default();
        c.stamp(Some(0.5), 480, 48000, 0);
        assert_eq!(c.stamp(None, 480, 48000, 0), None);
        assert_eq!(c.stamp(Some(0.52), 480, 48000, 0), Some(0.52));
    }
    #[test]
    fn valid_discontinuity_starts_new_run_without_compressing_missing_time() {
        let mut c = CaptureClock::default();
        c.stamp(Some(0.5), 480, 48000, 0);
        assert_eq!(c.stamp(Some(2.0), 480, 48000, 0), Some(2.0));
        assert_eq!(c.stamp(Some(2.011), 480, 48000, 0), Some(2.01));
        assert_eq!(c.stamp(Some(1.5), 480, 48000, 0), Some(1.5));
        assert_eq!(c.stamp(Some(1.511), 480, 48000, 0), Some(1.51));
    }
    #[test]
    fn resume_or_explicit_stream_reset_anchors_to_new_backend_time() {
        let mut c = CaptureClock::default();
        c.stamp(Some(0.5), 480, 48000, 0);
        assert_eq!(c.stamp(Some(0.8), 480, 48000, 1), Some(0.8));
        c.reset();
        assert_eq!(c.stamp(Some(1.2), 480, 48000, 1), Some(1.2));
    }
}
