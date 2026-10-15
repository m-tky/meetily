//! Sample-order alignment, not hardware-clock synchronization.
//! Selected sources share an output sample cursor. A stalled source is padded
//! only after a bounded wait; its late samples cannot be inserted a second time.
use super::recording_state::DeviceType;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

const INPUT_WAIT: Duration = Duration::from_millis(1200);
pub(crate) const LIVE_WINDOW_MS: u32 = 120;

#[derive(Default)]
struct SourceQueue {
    data: VecDeque<f32>,
    start: u64,
    received: u64,
}

#[derive(Default, Debug)]
pub(crate) struct AppendOutcome {
    pub late_samples: usize,
    pub overflow_samples: usize,
}

impl SourceQueue {
    fn append(&mut self, samples: Vec<f32>, cursor: u64, limit: usize) -> AppendOutcome {
        let incoming_start = self.received;
        self.received += samples.len() as u64;
        let skip = cursor
            .saturating_sub(incoming_start)
            .min(samples.len() as u64) as usize;
        if self.data.is_empty() {
            self.start = incoming_start + skip as u64;
        }
        self.data.extend(samples.into_iter().skip(skip));
        let overflow = self.data.len().saturating_sub(limit);
        self.data.drain(..overflow);
        self.start += overflow as u64;
        AppendOutcome {
            late_samples: skip,
            overflow_samples: overflow,
        }
    }

    fn take_interval(&mut self, cursor: u64, len: usize) -> Vec<f32> {
        // Overflow creates a gap, not a shift in the shared sample timeline.
        let mut out = vec![0.0; len];
        while self.start < cursor && !self.data.is_empty() {
            self.data.pop_front();
            self.start += 1;
        }
        let gap = self.start.saturating_sub(cursor).min(len as u64) as usize;
        let count = self.data.len().min(len - gap);
        for slot in &mut out[gap..gap + count] {
            *slot = self.data.pop_front().unwrap();
        }
        self.start += count as u64;
        out
    }
}

pub(crate) struct MixerWindow {
    pub mic: Vec<f32>,
    pub system: Vec<f32>,
    pub start_sample: u64,
    pub degraded_started: bool,
}

pub(crate) struct AudioMixerRingBuffer {
    mic: SourceQueue,
    system: SourceQueue,
    mic_enabled: bool,
    system_enabled: bool,
    window_size_samples: usize,
    max_buffer_size: usize,
    cursor: u64,
    waiting_since: Option<Instant>,
    degraded: bool,
}

impl AudioMixerRingBuffer {
    pub(crate) fn new(sample_rate: u32, mic_enabled: bool, system_enabled: bool) -> Self {
        Self::with_window(sample_rate, mic_enabled, system_enabled, 600)
    }

    pub(crate) fn with_window(
        sample_rate: u32,
        mic_enabled: bool,
        system_enabled: bool,
        window_ms: u32,
    ) -> Self {
        let window_size_samples = (sample_rate as usize * window_ms as usize / 1000).max(1);
        Self {
            mic: SourceQueue::default(),
            system: SourceQueue::default(),
            mic_enabled,
            system_enabled,
            window_size_samples,
            // Preserve the old 4.8s capacity independently of processing
            // cadence; shrinking it below the 1.2s wait would cause overflow.
            max_buffer_size: (sample_rate as usize * 4800 / 1000).max(1),
            cursor: 0,
            waiting_since: None,
            degraded: false,
        }
    }

    pub(crate) fn add_samples(&mut self, kind: DeviceType, samples: Vec<f32>) -> AppendOutcome {
        match kind {
            DeviceType::Microphone if self.mic_enabled => {
                self.mic.append(samples, self.cursor, self.max_buffer_size)
            }
            DeviceType::System if self.system_enabled => {
                self.system
                    .append(samples, self.cursor, self.max_buffer_size)
            }
            _ => AppendOutcome::default(),
        }
    }

    /// Regular output uses complete windows. Stop uses the exact remaining
    /// sample count, with no rounding up to a full window and no timeout wait.
    pub(crate) fn next_window(&mut self, now: Instant, finishing: bool) -> Option<MixerWindow> {
        let end = self.mic.received.max(self.system.received);
        let remaining = end.saturating_sub(self.cursor);
        if remaining == 0 {
            self.waiting_since = None;
            return None;
        }
        let len = if finishing {
            remaining.min(self.window_size_samples as u64) as usize
        } else {
            self.window_size_samples
        };
        if !finishing && remaining < len as u64 {
            return None;
        }
        let target = self.cursor + len as u64;
        let both_ready = (!self.mic_enabled || self.mic.received >= target)
            && (!self.system_enabled || self.system.received >= target);
        let mut degraded_started = false;
        if both_ready {
            self.degraded = false;
            self.waiting_since = None;
        } else if !finishing {
            // Degraded describes a warning episode, not permission to bypass
            // future waits. Each new interval must let a returning source catch
            // up before its samples become permanently late.
            let since = *self.waiting_since.get_or_insert(now);
            if now.saturating_duration_since(since) < INPUT_WAIT {
                return None;
            }
            degraded_started = !self.degraded;
            self.degraded = true;
            self.waiting_since = None;
        }
        let start_sample = self.cursor;
        let mic = self.mic.take_interval(self.cursor, len);
        let system = self.system.take_interval(self.cursor, len);
        self.cursor = target;
        Some(MixerWindow {
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
    #[test]
    fn short_windows_preserve_every_sample_and_original_buffer_capacity() {
        for window_ms in [600, LIVE_WINDOW_MS] {
            let mut r = AudioMixerRingBuffer::with_window(48000, true, true, window_ms);
            assert_eq!(r.max_buffer_size, 230400);
            let now = Instant::now();
            let data: Vec<f32> = (0..24001).map(|n| n as f32).collect();
            r.add_samples(DeviceType::Microphone, data.clone());
            assert!(r.next_window(now, false).is_none());
            r.add_samples(DeviceType::System, data.clone());
            let mut output = Vec::new();
            while let Some(w) = r.next_window(now, false) {
                assert_eq!(w.start_sample as usize, output.len());
                assert_eq!(w.mic, w.system);
                assert!(!w.degraded_started);
                output.extend(w.mic);
            }
            while let Some(w) = r.next_window(now, true) {
                assert_eq!(w.start_sample as usize, output.len());
                output.extend(w.mic);
            }
            assert_eq!(output, data);
        }
    }
    #[test]
    fn short_window_timeout_still_waits_1200ms_and_recovers() {
        let mut r = AudioMixerRingBuffer::with_window(48000, true, true, LIVE_WINDOW_MS);
        let now = Instant::now();
        r.add_samples(DeviceType::Microphone, vec![0.1; 11520]);
        assert!(r.next_window(now, false).is_none());
        assert!(r
            .next_window(now + INPUT_WAIT - Duration::from_millis(1), false)
            .is_none());
        assert!(
            r.next_window(now + INPUT_WAIT, false)
                .unwrap()
                .degraded_started
        );
        assert!(r.next_window(now + INPUT_WAIT, false).is_none());
        assert_eq!(
            r.add_samples(DeviceType::System, vec![0.2; 5760])
                .late_samples,
            5760
        );
        r.add_samples(DeviceType::System, vec![0.3; 5760]);
        assert_eq!(
            r.next_window(now + INPUT_WAIT, false).unwrap().system,
            vec![0.3; 5760]
        );
    }
    fn dual() -> AudioMixerRingBuffer {
        AudioMixerRingBuffer::new(48000, true, true)
    }
    #[test]
    fn waits_for_delayed_selected_source() {
        let mut r = dual();
        let now = Instant::now();
        r.add_samples(DeviceType::Microphone, vec![0.1; 28800]);
        assert!(r.next_window(now, false).is_none());
        r.add_samples(DeviceType::System, vec![0.2; 28800]);
        let w = r
            .next_window(now + Duration::from_millis(600), false)
            .unwrap();
        assert_eq!(w.mic, vec![0.1; 28800]);
        assert_eq!(w.system, vec![0.2; 28800]);
        assert_eq!(w.start_sample, 0);
        assert!(!w.degraded_started);
        assert!(r.next_window(now, false).is_none());
        assert_eq!(r.cursor, 28800);
    }
    #[test]
    fn returning_source_recovers_when_other_source_arrives_first() {
        for fast_is_mic in [true, false] {
            let mut r = dual();
            let now = Instant::now();
            let fast = || {
                if fast_is_mic {
                    DeviceType::Microphone
                } else {
                    DeviceType::System
                }
            };
            let slow = || {
                if fast_is_mic {
                    DeviceType::System
                } else {
                    DeviceType::Microphone
                }
            };
            r.add_samples(fast(), vec![0.1; 28800]);
            assert!(r.next_window(now, false).is_none());
            let first = r.next_window(now + INPUT_WAIT, false).unwrap();
            assert!(first.degraded_started);
            assert_eq!(first.start_sample, 0);
            r.add_samples(fast(), vec![0.1; 28800]);
            let returning = now + INPUT_WAIT + Duration::from_millis(1);
            assert!(
                r.next_window(returning, false).is_none(),
                "new interval must regain its wait"
            );
            assert_eq!(r.add_samples(slow(), vec![0.2; 28800]).late_samples, 28800);
            assert!(r.next_window(returning, false).is_none());
            assert_eq!(r.add_samples(slow(), vec![0.2; 28800]).late_samples, 0);
            let recovered = r
                .next_window(returning + Duration::from_millis(10), false)
                .unwrap();
            assert_eq!(recovered.start_sample, 28800);
            assert!(!recovered.degraded_started);
            assert_eq!(
                recovered.mic,
                vec![if fast_is_mic { 0.1 } else { 0.2 }; 28800]
            );
            assert_eq!(
                recovered.system,
                vec![if fast_is_mic { 0.2 } else { 0.1 }; 28800]
            );
            // Continue the previously failing callback order repeatedly.
            for interval in 2..6 {
                r.add_samples(fast(), vec![0.1; 28800]);
                assert!(r.next_window(returning, false).is_none());
                assert_eq!(r.add_samples(slow(), vec![0.2; 28800]).late_samples, 0);
                let w = r.next_window(returning, false).unwrap();
                assert_eq!(w.start_sample, interval * 28800);
                assert_eq!(w.mic, vec![if fast_is_mic { 0.1 } else { 0.2 }; 28800]);
                assert_eq!(w.system, vec![if fast_is_mic { 0.2 } else { 0.1 }; 28800]);
            }
        }
    }
    #[test]
    fn every_missing_interval_waits_but_warns_once_per_degraded_episode() {
        let mut r = dual();
        let now = Instant::now();
        r.add_samples(DeviceType::Microphone, vec![0.1; 57600]);
        assert!(r.next_window(now, false).is_none());
        assert!(
            r.next_window(now + INPUT_WAIT, false)
                .unwrap()
                .degraded_started
        );
        assert!(r.next_window(now + INPUT_WAIT, false).is_none());
        assert!(r
            .next_window(now + INPUT_WAIT * 2 - Duration::from_millis(1), false)
            .is_none());
        let second = r.next_window(now + INPUT_WAIT * 2, false).unwrap();
        assert!(!second.degraded_started);
        assert_eq!(second.start_sample, 28800);
        assert_eq!(second.system, vec![0.0; 28800]);
        assert!(r.next_window(now + INPUT_WAIT * 2, false).is_none());
    }
    #[test]
    fn stop_during_recovery_wait_flushes_exact_tail_without_waiting() {
        let mut r = dual();
        let now = Instant::now();
        r.add_samples(DeviceType::System, vec![0.2; 28800]);
        assert!(r.next_window(now, false).is_none());
        r.next_window(now + INPUT_WAIT, false).unwrap();
        r.add_samples(DeviceType::System, vec![0.3; 28801]);
        assert!(r.next_window(now + INPUT_WAIT, false).is_none());
        let full = r.next_window(now + INPUT_WAIT, true).unwrap();
        let tail = r.next_window(now + INPUT_WAIT, true).unwrap();
        assert_eq!(full.system, vec![0.3; 28800]);
        assert_eq!(tail.system, vec![0.3]);
        assert_eq!(tail.start_sample, 57600);
        assert!(r.next_window(now + INPUT_WAIT, true).is_none());
    }
    #[test]
    fn single_sources_do_not_wait_for_disabled_source() {
        for (mic, kind) in [(true, DeviceType::Microphone), (false, DeviceType::System)] {
            let mut r = AudioMixerRingBuffer::new(48000, mic, !mic);
            r.add_samples(kind, vec![0.25; 28800]);
            assert!(r.next_window(Instant::now(), false).is_some());
        }
    }
    #[test]
    fn no_inputs_or_no_data_do_not_invent_silence() {
        let mut r = AudioMixerRingBuffer::new(48000, false, false);
        r.add_samples(DeviceType::System, vec![1.0; 28800]);
        assert!(r.next_window(Instant::now(), true).is_none());
        assert!(dual().next_window(Instant::now(), true).is_none());
    }
    #[test]
    fn stop_preserves_exact_500ms_and_is_idempotent() {
        let mut r = dual();
        let now = Instant::now();
        r.add_samples(DeviceType::System, vec![0.3; 24000]);
        assert!(r.next_window(now, false).is_none());
        let w = r.next_window(now, true).unwrap();
        assert_eq!(w.system, vec![0.3; 24000]);
        assert_eq!(w.mic, vec![0.0; 24000]);
        assert_eq!(r.cursor, 24000);
        assert!(r.next_window(now, true).is_none());
    }
    #[test]
    fn unequal_stop_tails_keep_all_samples_without_window_padding() {
        let mut r = dual();
        let now = Instant::now();
        r.add_samples(DeviceType::Microphone, vec![0.1; 30001]);
        r.add_samples(DeviceType::System, vec![0.2; 31003]);
        let a = r.next_window(now, true).unwrap();
        let b = r.next_window(now, true).unwrap();
        assert_eq!(a.mic.len() + b.mic.len(), 31003);
        assert_eq!(b.start_sample, 28800);
        assert_eq!(b.system, vec![0.2; 2203]);
        assert_eq!(&b.mic[..1201], &vec![0.1; 1201]);
        assert_eq!(&b.mic[1201..], &vec![0.0; 1002]);
        assert!(r.next_window(now, true).is_none());
    }
    #[test]
    fn timeout_is_bounded_and_warns_once_per_outage() {
        let mut r = dual();
        let now = Instant::now();
        r.add_samples(DeviceType::Microphone, vec![0.1; 57600]);
        assert!(r.next_window(now, false).is_none());
        assert!(r
            .next_window(now + Duration::from_millis(1199), false)
            .is_none());
        let a = r.next_window(now + INPUT_WAIT, false).unwrap();
        assert!(a.degraded_started);
        assert!(r.next_window(now + INPUT_WAIT, false).is_none());
        let b = r.next_window(now + INPUT_WAIT * 2, false).unwrap();
        assert!(!b.degraded_started);
        assert_eq!(b.start_sample, 28800);
        assert!(b.system.iter().all(|x| *x == 0.0));
        assert_eq!(r.cursor, 57600);
        assert!(r.next_window(now + INPUT_WAIT * 2, false).is_none());
    }
    #[test]
    fn late_samples_are_discarded_not_reinserted_and_recovery_aligns() {
        let mut r = dual();
        let now = Instant::now();
        r.add_samples(DeviceType::Microphone, vec![0.1; 28800]);
        assert!(r.next_window(now, false).is_none());
        r.next_window(now + INPUT_WAIT, false).unwrap();
        let dropped = r.add_samples(DeviceType::System, vec![0.8; 28800]);
        assert_eq!(dropped.late_samples, 28800);
        assert!(r.next_window(now + INPUT_WAIT, false).is_none());
        r.add_samples(DeviceType::System, vec![0.2; 28800]);
        r.add_samples(DeviceType::Microphone, vec![0.3; 28800]);
        let w = r.next_window(now + INPUT_WAIT, false).unwrap();
        assert_eq!(w.system, vec![0.2; 28800]);
        assert_eq!(w.mic, vec![0.3; 28800]);
        assert_eq!(w.start_sample, 28800);
        assert!(!r.degraded);
    }
    #[test]
    fn partial_late_chunk_keeps_only_future_samples() {
        let mut r = dual();
        let now = Instant::now();
        r.add_samples(DeviceType::Microphone, vec![0.1; 28800]);
        assert!(r.next_window(now, false).is_none());
        r.next_window(now + INPUT_WAIT, false).unwrap();
        let mut sys = vec![0.9; 28800];
        sys.extend(vec![0.2; 28800]);
        assert_eq!(r.add_samples(DeviceType::System, sys).late_samples, 28800);
        r.add_samples(DeviceType::Microphone, vec![0.3; 28800]);
        assert_eq!(r.next_window(now, false).unwrap().system, vec![0.2; 28800]);
    }
    #[test]
    fn overflow_gap_does_not_shift_surviving_samples() {
        let mut r = AudioMixerRingBuffer::new(48000, false, true);
        let now = Instant::now();
        let limit = r.max_buffer_size;
        let result = r.add_samples(DeviceType::System, vec![0.25; limit + 123]);
        assert_eq!(result.overflow_samples, 123);
        assert_eq!(r.system.data.len(), limit);
        let w = r.next_window(now, false).unwrap();
        assert_eq!(&w.system[..123], &vec![0.0; 123]);
        assert!(w.system[123..].iter().all(|x| *x == 0.25));
        assert_eq!(w.start_sample, 0);
    }
    #[test]
    fn irregular_chunking_preserves_sample_order_and_length() {
        let mut r = dual();
        let now = Instant::now();
        let mic: Vec<f32> = (0..70001).map(|i| i as f32 / 100000.0).collect();
        let sys: Vec<f32> = mic.iter().map(|x| -x).collect();
        let mut mo = Vec::new();
        let mut so = Vec::new();
        for (m, s) in mic.chunks(997).zip(sys.chunks(997)) {
            r.add_samples(DeviceType::Microphone, m.to_vec());
            while let Some(w) = r.next_window(now, false) {
                mo.extend(w.mic);
                so.extend(w.system);
            }
            r.add_samples(DeviceType::System, s.to_vec());
            while let Some(w) = r.next_window(now, false) {
                mo.extend(w.mic);
                so.extend(w.system);
            }
        }
        while let Some(w) = r.next_window(now, true) {
            mo.extend(w.mic);
            so.extend(w.system);
        }
        assert_eq!(mo, mic);
        assert_eq!(so, sys);
    }
    #[test]
    fn timeout_wait_resets_after_both_inputs_recover() {
        let mut r = dual();
        let now = Instant::now();
        r.add_samples(DeviceType::System, vec![0.2; 28800]);
        assert!(r.next_window(now, false).is_none());
        r.next_window(now + INPUT_WAIT, false).unwrap();
        r.add_samples(DeviceType::Microphone, vec![0.1; 57600]);
        r.add_samples(DeviceType::System, vec![0.2; 28800]);
        r.next_window(now + INPUT_WAIT, false).unwrap();
        r.add_samples(DeviceType::System, vec![0.2; 28800]);
        assert!(r.next_window(now + INPUT_WAIT, false).is_none());
        assert!(r
            .next_window(now + INPUT_WAIT + Duration::from_millis(1199), false)
            .is_none());
        assert!(
            r.next_window(now + INPUT_WAIT + INPUT_WAIT, false)
                .unwrap()
                .degraded_started
        );
    }
}
