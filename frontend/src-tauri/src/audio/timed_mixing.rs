//! Capture-time placement on the session's 48kHz timeline. Arrival time is not
//! a source timestamp. Sparse blocks retain startup/gap silence without allocating
//! it. The original sample-order mixer remains available for legacy callers.
use super::recording_state::DeviceType;
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};
const WAIT: Duration = Duration::from_millis(1200);
const RATE: f64 = 48000.0;
// Backend timing estimates have sub-block jitter. Do not drop/insert audio for
// <=5ms of measurement noise; larger discontinuities remain explicit gaps/overlap.
const JITTER_FRAMES: i64 = 240;
const BUFFER_LIMIT: usize = 28800 * 8;

struct Block {
    start: u64,
    data: VecDeque<f32>,
}
#[derive(Default)]
struct Source {
    blocks: VecDeque<Block>,
    end: u64,
    initialized: bool,
    queued: usize,
}
#[derive(Default, Debug)]
pub(crate) struct TimedAppend {
    pub late: usize,
    pub overlap: usize,
    pub overflow: usize,
    pub gap: u64,
    pub invalid: bool,
}
impl Source {
    fn append(&mut self, mut start: i64, data: Vec<f32>, cursor: u64) -> TimedAppend {
        let mut out = TimedAppend::default();
        if data.is_empty() {
            return out;
        }
        // Snap only adjacent backend estimates, never a late source's initial
        // packet or a real startup offset to the output cursor.
        if self.initialized && (start - self.end as i64).abs() <= JITTER_FRAMES {
            start = self.end as i64;
        }
        let original_end = start.saturating_add(data.len() as i64).max(0) as u64;
        let skip_negative = start.saturating_neg().max(0) as usize;
        let skip_late = (cursor as i64 - start.max(0)).max(0) as usize;
        let already = if self.initialized { self.end } else { 0 };
        let skip_overlap = already
            .saturating_sub(start.max(0) as u64)
            .min(data.len() as u64) as usize;
        let skip = skip_negative
            .saturating_add(skip_late.max(skip_overlap))
            .min(data.len());
        out.late = skip_negative.saturating_add(skip_late).min(data.len());
        out.overlap = skip.saturating_sub(out.late);
        let kept_start = (start + skip as i64).max(0) as u64;
        if self.initialized {
            out.gap = kept_start.saturating_sub(self.end.max(cursor));
        }
        if skip < data.len() {
            let data: VecDeque<_> = data.into_iter().skip(skip).collect();
            self.queued += data.len();
            self.blocks.push_back(Block {
                start: kept_start,
                data,
            });
        }
        self.end = self.end.max(original_end);
        self.initialized = true;
        while self.queued > BUFFER_LIMIT {
            let excess = self.queued - BUFFER_LIMIT;
            let front = self.blocks.front_mut().unwrap();
            let n = excess.min(front.data.len());
            front.data.drain(..n);
            front.start += n as u64;
            self.queued -= n;
            out.overflow += n;
            if front.data.is_empty() {
                self.blocks.pop_front();
            }
        }
        out
    }
    fn take(&mut self, cursor: u64, len: usize) -> Vec<f32> {
        let mut out = vec![0.0; len];
        let target = cursor + len as u64;
        while let Some(block) = self.blocks.front_mut() {
            if block.start >= target {
                break;
            }
            let skip = cursor
                .saturating_sub(block.start)
                .min(block.data.len() as u64) as usize;
            block.data.drain(..skip);
            block.start += skip as u64;
            self.queued -= skip;
            if !block.data.is_empty() {
                let offset = block.start.saturating_sub(cursor) as usize;
                let n = block.data.len().min(len - offset);
                for slot in &mut out[offset..offset + n] {
                    *slot = block.data.pop_front().unwrap();
                }
                block.start += n as u64;
                self.queued -= n;
            }
            if block.data.is_empty() {
                self.blocks.pop_front();
            } else {
                break;
            }
        }
        out
    }
}
pub(crate) struct TimedWindow {
    pub mic: Vec<f32>,
    pub system: Vec<f32>,
    pub start_sample: u64,
    pub degraded_started: bool,
}
pub(crate) struct TimedMixer {
    mic: Source,
    system: Source,
    mic_enabled: bool,
    system_enabled: bool,
    cursor: u64,
    waiting: Option<Instant>,
    degraded: bool,
    window_size: usize,
}
impl TimedMixer {
    pub fn new(mic_enabled: bool, system_enabled: bool) -> Self {
        Self::with_window(mic_enabled, system_enabled, 600)
    }
    pub fn with_window(mic_enabled: bool, system_enabled: bool, window_ms: u32) -> Self {
        Self {
            mic: Source::default(),
            system: Source::default(),
            mic_enabled,
            system_enabled,
            cursor: 0,
            waiting: None,
            degraded: false,
            window_size: (48 * window_ms as usize).max(1),
        }
    }
    pub fn append(
        &mut self,
        kind: DeviceType,
        start_s: Option<f64>,
        samples: Vec<f32>,
    ) -> TimedAppend {
        let Some(time) =
            start_s.filter(|t| t.is_finite() && *t >= -10.0 && *t <= (u64::MAX / 2) as f64 / RATE)
        else {
            return TimedAppend {
                invalid: true,
                ..Default::default()
            };
        };
        let start = (time * RATE).round() as i64;
        let source = match kind {
            DeviceType::Microphone if self.mic_enabled => &mut self.mic,
            DeviceType::System if self.system_enabled => &mut self.system,
            _ => return TimedAppend::default(),
        };
        source.append(start, samples, self.cursor)
    }
    pub fn next_window(&mut self, now: Instant, finishing: bool) -> Option<TimedWindow> {
        let end = self.mic.end.max(self.system.end);
        let remaining = end.saturating_sub(self.cursor);
        if remaining == 0 {
            self.waiting = None;
            return None;
        }
        let len = if finishing {
            remaining.min(self.window_size as u64) as usize
        } else {
            self.window_size
        };
        if remaining < len as u64 {
            return None;
        }
        let target = self.cursor + len as u64;
        let ready = (!self.mic_enabled || (self.mic.initialized && self.mic.end >= target))
            && (!self.system_enabled || (self.system.initialized && self.system.end >= target));
        let mut degraded_started = false;
        if ready {
            self.waiting = None;
            self.degraded = false;
        } else if !finishing {
            let since = *self.waiting.get_or_insert(now);
            if now.saturating_duration_since(since) < WAIT {
                return None;
            }
            degraded_started = !self.degraded;
            self.degraded = true;
            self.waiting = None;
        }
        let start_sample = self.cursor;
        let mic = self.mic.take(self.cursor, len);
        let system = self.system.take(self.cursor, len);
        self.cursor = target;
        Some(TimedWindow {
            mic,
            system,
            start_sample,
            degraded_started,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use DeviceType::{Microphone as M, System as S};
    #[test]
    fn short_windows_preserve_timestamp_positions_and_exact_stop() {
        let now = Instant::now();
        for window_ms in [600, 120] {
            let mut r = TimedMixer::with_window(true, true, window_ms);
            r.append(M, Some(0.0), vec![0.1; 24001]);
            r.append(S, Some(0.02), vec![0.2; 23041]);
            let mut mic = Vec::new();
            let mut pc = Vec::new();
            while let Some(w) = r.next_window(now, false) {
                assert_eq!(w.start_sample as usize, mic.len());
                assert_eq!(w.mic.len(), 48 * window_ms as usize);
                assert!(!w.degraded_started);
                mic.extend(w.mic);
                pc.extend(w.system);
            }
            while let Some(w) = r.next_window(now, true) {
                mic.extend(w.mic);
                pc.extend(w.system);
            }
            assert_eq!(mic, vec![0.1; 24001]);
            assert_eq!(pc.len(), 24001);
            assert_eq!(pc[..960], vec![0.0; 960]);
            assert_eq!(pc[960..], vec![0.2; 23041]);
        }
    }
    #[test]
    fn valid_clock_discontinuity_preserves_gap_and_returning_samples() {
        let now = Instant::now();
        let mut c = super::super::capture_clock::CaptureClock::default();
        let mut r = TimedMixer::with_window(true, false, 120);
        r.append(M, c.stamp(Some(0.0), 4800, 48000, 0), vec![0.1; 4800]);
        let a = r.append(M, c.stamp(Some(0.3), 4800, 48000, 0), vec![0.2; 4800]);
        assert_eq!(a.gap, 9600);
        assert!(!a.invalid);
        let mut out = Vec::new();
        while let Some(w) = r.next_window(now, true) {
            out.extend(w.mic);
        }
        assert_eq!(out.len(), 19200);
        assert_eq!(out[..4800], vec![0.1; 4800]);
        assert_eq!(out[4800..14400], vec![0.0; 9600]);
        assert_eq!(out[14400..], vec![0.2; 4800]);
    }
    #[test]
    fn delayed_first_source_keeps_capture_position_not_sample_zero() {
        let now = Instant::now();
        let mut r = TimedMixer::new(true, true);
        r.append(S, Some(0.6), vec![0.2; 48000]);
        assert!(r.next_window(now, false).is_none());
        let a = r.append(M, Some(1.85), vec![0.3; 4800]);
        assert_eq!(a.late, 0);
        let w = r
            .next_window(now + Duration::from_millis(700), false)
            .unwrap();
        assert!(!w.degraded_started);
        assert!(w.mic.iter().all(|v| *v == 0.0));
        let mut mic = w.mic;
        while let Some(w) = r.next_window(now, true) {
            mic.extend(w.mic);
        }
        assert_eq!(mic.iter().position(|v| *v != 0.0), Some(88800));
        assert_eq!(mic.iter().filter(|v| **v == 0.3).count(), 4800);
    }
    #[test]
    fn timestamped_hole_is_silence_not_time_compression() {
        let mut r = TimedMixer::new(true, false);
        let now = Instant::now();
        r.append(M, Some(0.0), vec![0.1; 4800]);
        let a = r.append(M, Some(0.3), vec![0.2; 4800]);
        assert_eq!(a.gap, 9600);
        let w = r.next_window(now, true).unwrap();
        assert_eq!(w.mic.len(), 19200);
        assert!(w.mic[4800..14400].iter().all(|v| *v == 0.0));
    }
    #[test]
    fn timeout_recovery_waits_again_and_does_not_shift_late_audio() {
        let mut r = TimedMixer::new(true, true);
        let now = Instant::now();
        r.append(S, Some(0.0), vec![0.1; 57600]);
        assert!(r.next_window(now, false).is_none());
        assert!(r.next_window(now + WAIT, false).unwrap().degraded_started);
        assert!(r.next_window(now + WAIT, false).is_none());
        assert_eq!(r.append(M, Some(0.0), vec![0.2; 28800]).late, 28800);
        assert_eq!(r.append(M, Some(0.6), vec![0.3; 28800]).late, 0);
        let w = r.next_window(now + WAIT, false).unwrap();
        assert!(!w.degraded_started);
        assert_eq!(w.mic[0], 0.3);
    }
    #[test]
    fn unknown_or_invalid_timestamps_never_become_arrival_or_zero_time() {
        let mut r = TimedMixer::new(true, false);
        for t in [None, Some(f64::NAN), Some(f64::INFINITY)] {
            assert!(r.append(M, t, vec![1.0; 480]).invalid);
        }
        assert!(r.next_window(Instant::now(), true).is_none());
    }
    #[test]
    fn stop_tail_exact_and_unequal_sources() {
        let mut r = TimedMixer::new(true, true);
        let now = Instant::now();
        r.append(M, Some(0.0), vec![0.1; 1001]);
        r.append(S, Some(0.02), vec![0.2; 1000]);
        let w = r.next_window(now, true).unwrap();
        assert_eq!(w.mic.len(), 1960);
        assert_eq!(w.system.iter().filter(|v| **v == 0.2).count(), 1000);
        assert!(!w.degraded_started);
        assert!(r.next_window(now, true).is_none());
    }
    #[test]
    fn backend_jitter_does_not_delete_adjacent_speech() {
        let mut r = TimedMixer::new(true, false);
        r.append(M, Some(0.0), vec![0.1; 4800]);
        let a = r.append(M, Some(0.102), vec![0.2; 4800]);
        assert_eq!(a.gap, 0);
        assert_eq!(a.overlap, 0);
        assert_eq!(r.next_window(Instant::now(), true).unwrap().mic.len(), 9600);
    }
    #[test]
    fn real_overlap_is_not_mixed_twice() {
        let mut r = TimedMixer::new(true, false);
        r.append(M, Some(0.0), vec![0.1; 4800]);
        assert_eq!(r.append(M, Some(0.05), vec![0.2; 4800]).overlap, 2400);
        assert_eq!(r.next_window(Instant::now(), true).unwrap().mic.len(), 7200);
    }
    #[test]
    fn sparse_startup_and_overflow_are_bounded() {
        let mut r = TimedMixer::new(true, false);
        let a = r.append(M, Some(1000.0), vec![0.1; BUFFER_LIMIT + 100]);
        assert_eq!(a.overflow, 100);
        assert_eq!(r.mic.queued, BUFFER_LIMIT);
        assert_eq!(r.mic.blocks.len(), 1);
    }
}
