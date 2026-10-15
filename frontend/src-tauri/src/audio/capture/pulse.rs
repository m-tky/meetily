//! Linux native libpulse monitor capture (also PipeWire's Pulse server).
//! All C pointers/callbacks stay on one owner thread. Nonblocking mainloop
//! polling permits bounded stop even when the server/source disappears.
//! ABI declarations below are the stable public libpulse C API, not private
//! PipeWire layouts. No pactl/parec subprocess or shell command is used by app.
use anyhow::{bail, ensure, Result};
use std::{
    ffi::{c_char, c_int, c_void, CStr, CString},
    ptr,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    thread,
    time::{Duration, Instant},
};

pub const PREFIX: &str = "pulse-monitor:";
pub const INPUT_PREFIX: &str = "pulse-input:";
const RATE: u32 = 48000;
const LIMIT: Duration = Duration::from_secs(5);
/// Explicit transport policy. Private A/B tests retain FragmentOnly as the
/// baseline; AdjustSource requests low source latency for live capture.
#[derive(Clone, Copy)]
pub enum MonitorLatencyMode {
    FragmentOnly,
    AdjustSource,
}
impl MonitorLatencyMode {
    fn flags(self) -> u32 {
        0x0200
            | match self {
                Self::FragmentOnly => 0,
                Self::AdjustSource => 0x2000, // PA_STREAM_ADJUST_LATENCY
            }
    }
}
// Request 100ms delivery, not a hardware/source latency or buffer-cap change.
// Keep all other attributes server-selected, as with the previous NULL attrs.
#[repr(C)]
struct BufferAttr {
    maxlength: u32,
    tlength: u32,
    prebuf: u32,
    minreq: u32,
    fragsize: u32,
}
fn monitor_buffer_attr() -> BufferAttr {
    BufferAttr {
        maxlength: u32::MAX,
        tlength: u32::MAX,
        prebuf: u32::MAX,
        minreq: u32::MAX,
        fragsize: RATE / 10 * std::mem::size_of::<f32>() as u32,
    }
}
#[repr(C)]
struct SampleSpec {
    format: c_int,
    rate: u32,
    channels: u8,
}
#[repr(C)]
struct ChannelMap {
    channels: u8,
    map: [c_int; 32],
}
#[repr(C)]
struct Volume {
    channels: u8,
    values: [u32; 32],
}
// Only the stable prefix is read. Never allocate/return this truncated struct.
#[repr(C)]
struct SourceInfo {
    name: *const c_char,
    index: u32,
    description: *const c_char,
    sample_spec: SampleSpec,
    channel_map: ChannelMap,
    owner_module: u32,
    volume: Volume,
    mute: c_int,
    monitor_of_sink: u32,
}
#[repr(C)]
struct ServerInfo {
    user_name: *const c_char,
    host_name: *const c_char,
    server_version: *const c_char,
    server_name: *const c_char,
    sample_spec: SampleSpec,
    default_sink_name: *const c_char,
}
type Opaque = c_void;
#[link(name = "pulse")]
extern "C" {
    fn pa_mainloop_new() -> *mut Opaque;
    fn pa_mainloop_free(m: *mut Opaque);
    fn pa_mainloop_get_api(m: *mut Opaque) -> *mut Opaque;
    fn pa_mainloop_iterate(m: *mut Opaque, block: c_int, retval: *mut c_int) -> c_int;
    fn pa_context_new(api: *mut Opaque, name: *const c_char) -> *mut Opaque;
    fn pa_context_connect(
        c: *mut Opaque,
        server: *const c_char,
        flags: u32,
        api: *const Opaque,
    ) -> c_int;
    fn pa_context_get_state(c: *const Opaque) -> c_int;
    fn pa_context_disconnect(c: *mut Opaque);
    fn pa_context_unref(c: *mut Opaque);
    fn pa_context_get_source_info_list(
        c: *mut Opaque,
        cb: unsafe extern "C" fn(*mut Opaque, *const SourceInfo, c_int, *mut Opaque),
        data: *mut Opaque,
    ) -> *mut Opaque;
    fn pa_context_get_server_info(
        c: *mut Opaque,
        cb: unsafe extern "C" fn(*mut Opaque, *const ServerInfo, *mut Opaque),
        data: *mut Opaque,
    ) -> *mut Opaque;
    fn pa_operation_get_state(o: *const Opaque) -> c_int;
    fn pa_operation_cancel(o: *mut Opaque);
    fn pa_operation_unref(o: *mut Opaque);
    fn pa_stream_new(
        c: *mut Opaque,
        name: *const c_char,
        spec: *const SampleSpec,
        map: *const Opaque,
    ) -> *mut Opaque;
    fn pa_stream_connect_record(
        s: *mut Opaque,
        dev: *const c_char,
        attr: *const BufferAttr,
        flags: u32,
    ) -> c_int;
    fn pa_stream_get_buffer_attr(s: *const Opaque) -> *const BufferAttr;
    fn pa_stream_update_timing_info(
        s: *mut Opaque,
        cb: Option<unsafe extern "C" fn(*mut Opaque, c_int, *mut Opaque)>,
        data: *mut Opaque,
    ) -> *mut Opaque;
    fn pa_stream_get_latency(s: *mut Opaque, usec: *mut u64, negative: *mut c_int) -> c_int;
    fn pa_stream_get_state(s: *const Opaque) -> c_int;
    fn pa_stream_peek(s: *mut Opaque, data: *mut *const Opaque, bytes: *mut usize) -> c_int;
    fn pa_stream_drop(s: *mut Opaque) -> c_int;
    fn pa_stream_disconnect(s: *mut Opaque) -> c_int;
    fn pa_stream_unref(s: *mut Opaque);
}
struct Session {
    mainloop: *mut Opaque,
    context: *mut Opaque,
    stream: *mut Opaque,
}
impl Drop for Session {
    fn drop(&mut self) {
        unsafe {
            if !self.stream.is_null() {
                pa_stream_disconnect(self.stream);
                pa_stream_unref(self.stream);
            }
            if !self.context.is_null() {
                pa_context_disconnect(self.context);
                pa_context_unref(self.context);
            }
            if !self.mainloop.is_null() {
                pa_mainloop_free(self.mainloop);
            }
        }
    }
}
impl Session {
    fn connect(stop: &AtomicBool) -> Result<Self> {
        let mut session = Self {
            mainloop: ptr::null_mut(),
            context: ptr::null_mut(),
            stream: ptr::null_mut(),
        };
        unsafe {
            session.mainloop = pa_mainloop_new();
            ensure!(!session.mainloop.is_null(), "Cannot create Pulse mainloop");
            session.context = pa_context_new(
                pa_mainloop_get_api(session.mainloop),
                b"Meetily local monitor\0".as_ptr().cast(),
            );
            ensure!(!session.context.is_null(), "Cannot create Pulse context");
            // NOAUTOSPAWN: never start another server or pick another backend.
            ensure!(
                pa_context_connect(session.context, ptr::null(), 1, ptr::null()) >= 0,
                "Cannot connect to PulseAudio/PipeWire server"
            );
        }
        session.wait(stop, || unsafe {
            match pa_context_get_state(session.context) {
                4 => Ok(true),
                5 | 6 => anyhow::bail!("Pulse server disconnected"),
                _ => Ok(false),
            }
        })?;
        Ok(session)
    }
    fn tick(&self) -> Result<()> {
        ensure!(
            unsafe { pa_mainloop_iterate(self.mainloop, 0, ptr::null_mut()) } >= 0,
            "Pulse mainloop failed"
        );
        Ok(())
    }
    fn wait(&self, stop: &AtomicBool, mut ready: impl FnMut() -> Result<bool>) -> Result<()> {
        let start = Instant::now();
        loop {
            ensure!(!stop.load(Ordering::Acquire), "Pulse capture cancelled");
            self.tick()?;
            if ready()? {
                return Ok(());
            }
            ensure!(start.elapsed() < LIMIT, "Pulse operation timed out");
            thread::sleep(Duration::from_millis(5));
        }
    }
    fn sources(&self, stop: &AtomicBool) -> Result<Vec<String>> {
        self.source_names(stop, false)
    }
    fn source_names(&self, stop: &AtomicBool, input: bool) -> Result<Vec<String>> {
        struct Sources {
            input: bool,
            names: Vec<String>,
            done: bool,
            failed: bool,
        }
        unsafe extern "C" fn callback(
            _: *mut Opaque,
            info: *const SourceInfo,
            eol: c_int,
            data: *mut Opaque,
        ) {
            let result = &*(data as *const std::cell::RefCell<Sources>);
            let mut result = result.borrow_mut();
            if eol != 0 {
                result.done = true;
                result.failed = eol < 0;
                return;
            }
            if let Some(info) = info.as_ref() {
                if (info.monitor_of_sink == u32::MAX) == result.input && !info.name.is_null() {
                    if let Ok(name) = CStr::from_ptr(info.name).to_str() {
                        result.names.push(name.to_owned());
                    }
                }
            }
        }
        let result = std::cell::RefCell::new(Sources {
            input,
            names: vec![],
            done: false,
            failed: false,
        });
        let op = unsafe {
            pa_context_get_source_info_list(
                self.context,
                callback,
                (&result as *const std::cell::RefCell<Sources>)
                    .cast_mut()
                    .cast(),
            )
        };
        ensure!(!op.is_null(), "Cannot enumerate Pulse monitor sources");
        // Cancel before stack callback userdata goes out of scope, including timeout.
        let waited = self.wait(stop, || {
            ensure!(
                unsafe { pa_context_get_state(self.context) } == 4,
                "Pulse server disconnected during enumeration"
            );
            let result = result.borrow();
            ensure!(
                unsafe { pa_operation_get_state(op) } == 0 || result.done,
                "Pulse enumeration cancelled"
            );
            Ok(result.done)
        });
        unsafe {
            pa_operation_cancel(op);
            pa_operation_unref(op);
        }
        waited?;
        let result = result.into_inner();
        ensure!(!result.failed, "Pulse monitor enumeration failed");
        Ok(result.names)
    }
    fn record(
        &mut self,
        source: &str,
        stop: &AtomicBool,
        mode: MonitorLatencyMode,
        input: bool,
    ) -> Result<()> {
        ensure!(
            self.source_names(stop, input)?.iter().any(|n| n == source),
            "Selected Pulse monitor is unavailable; no source fallback"
        );
        let name = CString::new(source)?;
        // PA_SAMPLE_FLOAT32LE = 5. Pulse converts/remixes the selected source
        // to mono 48k. Decode bytes explicitly so alignment isn't assumed.
        let spec = SampleSpec {
            format: 5,
            rate: RATE,
            channels: 1,
        };
        unsafe {
            self.stream = pa_stream_new(
                self.context,
                b"Meetily system audio\0".as_ptr().cast(),
                &spec,
                ptr::null(),
            );
            ensure!(
                !self.stream.is_null(),
                "Cannot create Pulse recording stream"
            );
            // DONT_MOVE: source disappearance must not move capture to a mic
            // or another monitor. User must reconnect/select explicitly.
            // Source latency adjustment is explicit, never silently enabled by
            // the fragment request. Keep DONT_MOVE in both modes.
            let attr = monitor_buffer_attr();
            ensure!(
                pa_stream_connect_record(self.stream, name.as_ptr(), &attr, mode.flags()) >= 0,
                "Cannot connect selected Pulse monitor"
            );
        }
        self.wait(stop, || unsafe {
            match pa_stream_get_state(self.stream) {
                2 => Ok(true),
                3 | 4 => bail!("Selected Pulse monitor failed"),
                _ => Ok(false),
            }
        })?;
        // Server may clamp the request; report only non-content numeric attrs.
        let actual = unsafe { pa_stream_get_buffer_attr(self.stream).as_ref() };
        if let Some(actual) = actual {
            log::info!(
                "{} capture buffer: requested_fragment_bytes={}, negotiated_fragment_bytes={}, negotiated_maxlength_bytes={}",
                if input { "Native Pulse input" } else { "PC Pulse" }, monitor_buffer_attr().fragsize, actual.fragsize, actual.maxlength
            );
        }
        Ok(())
    }
    /// Fresh server timing, at the record read head BEFORE drop advances it.
    /// No interpolation, wall-clock epoch conversion, or callback-time fallback.
    fn capture_head_time(&self, stop: &AtomicBool) -> Option<Instant> {
        let requested = Instant::now();
        let op = unsafe { pa_stream_update_timing_info(self.stream, None, ptr::null_mut()) };
        if op.is_null() {
            return None;
        }
        let valid = (|| -> Result<()> {
            loop {
                ensure!(!stop.load(Ordering::Acquire), "Pulse capture cancelled");
                self.tick()?;
                let state = unsafe { pa_operation_get_state(op) };
                if state != 0 {
                    ensure!(state == 1, "Pulse timing cancelled");
                    break;
                }
                ensure!(
                    requested.elapsed() < Duration::from_millis(100),
                    "Pulse timing stale"
                );
                thread::sleep(Duration::from_millis(1));
            }
            Ok(())
        })();
        unsafe {
            pa_operation_cancel(op);
            pa_operation_unref(op);
        }
        if valid.is_err() || requested.elapsed() > Duration::from_millis(100) {
            return None;
        }
        let mut usec = 0;
        let mut negative = 0;
        if unsafe { pa_stream_get_latency(self.stream, &mut usec, &mut negative) } < 0
            || usec > 10_000_000
        {
            return None;
        }
        let age = Duration::from_micros(usec);
        if negative != 0 {
            Instant::now().checked_add(age)
        } else {
            Instant::now().checked_sub(age)
        }
    }
    fn read(
        &self,
        stop: &AtomicBool,
        timed: bool,
        consume: &mut impl FnMut(&[f32], Option<Instant>),
    ) -> Result<()> {
        self.tick()?;
        ensure!(
            unsafe { pa_context_get_state(self.context) } == 4
                && unsafe { pa_stream_get_state(self.stream) } == 2,
            "Pulse monitor/server disconnected; select it again to reconnect"
        );
        let mut data = ptr::null();
        let mut len = 0;
        ensure!(
            unsafe { pa_stream_peek(self.stream, &mut data, &mut len) } >= 0,
            "Pulse monitor read failed"
        );
        if len == 0 {
            return Ok(());
        }
        ensure!(
            len % 4 == 0 && len <= 48000 * 4 * 10,
            "Invalid/oversized Pulse sample buffer"
        );
        let samples = if data.is_null() {
            // Pulse holes represent missing/silent data, not end of stream.
            vec![0.0; len / 4]
        } else {
            unsafe { std::slice::from_raw_parts(data.cast::<u8>(), len) }
                .chunks_exact(4)
                .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
                .collect()
        };
        let captured_at = if timed {
            self.capture_head_time(stop)
        } else {
            None
        };
        let dropped = unsafe { pa_stream_drop(self.stream) };
        ensure!(dropped >= 0, "Pulse monitor buffer release failed");
        ensure!(
            samples.iter().all(|x| x.is_finite()),
            "Nonfinite Pulse samples"
        );
        consume(&samples, captured_at);
        Ok(())
    }
}
pub fn input_names() -> Result<Vec<String>> {
    let stop = AtomicBool::new(false);
    Session::connect(&stop)?
        .source_names(&stop, true)
        .map(|names| {
            names
                .into_iter()
                .map(|n| format!("{INPUT_PREFIX}{n}"))
                .collect()
        })
}

pub fn monitor_names() -> Result<Vec<String>> {
    let stop = AtomicBool::new(false);
    Session::connect(&stop)?
        .sources(&stop)
        .map(|names| names.into_iter().map(|n| format!("{PREFIX}{n}")).collect())
}
/// Resolve the default *sink monitor*, never the default input/microphone.
/// Only called for the existing UI's explicit Default System Audio option.
pub fn default_monitor_name() -> Result<String> {
    let stop = AtomicBool::new(false);
    let session = Session::connect(&stop)?;
    let result = std::cell::RefCell::new(None::<Option<String>>);
    unsafe extern "C" fn callback(_: *mut Opaque, info: *const ServerInfo, data: *mut Opaque) {
        let result = &*(data as *const std::cell::RefCell<Option<Option<String>>>);
        let name = info.as_ref().and_then(|i| {
            if i.default_sink_name.is_null() {
                None
            } else {
                CStr::from_ptr(i.default_sink_name)
                    .to_str()
                    .ok()
                    .map(str::to_owned)
            }
        });
        *result.borrow_mut() = Some(name);
    }
    let op = unsafe {
        pa_context_get_server_info(
            session.context,
            callback,
            (&result as *const std::cell::RefCell<Option<Option<String>>>)
                .cast_mut()
                .cast(),
        )
    };
    ensure!(!op.is_null(), "Cannot query default Pulse sink");
    let waited = session.wait(&stop, || {
        ensure!(
            unsafe { pa_context_get_state(session.context) } == 4,
            "Pulse server disconnected"
        );
        Ok(result.borrow().is_some())
    });
    unsafe {
        pa_operation_cancel(op);
        pa_operation_unref(op);
    }
    waited?;
    let sink = result
        .into_inner()
        .flatten()
        .ok_or_else(|| anyhow::anyhow!("No default Pulse sink"))?;
    let monitor = format!("{sink}.monitor");
    ensure!(
        session.sources(&stop)?.contains(&monitor),
        "Default sink has no available Pulse monitor"
    );
    Ok(format!("{PREFIX}{monitor}"))
}
pub fn source_name(device: &str) -> Result<&str> {
    let name = device
        .strip_prefix(PREFIX)
        .ok_or_else(|| anyhow::anyhow!("Select a Pulse monitor device"))?;
    ensure!(
        !name.is_empty() && !name.contains('\0'),
        "Invalid Pulse monitor identity"
    );
    Ok(name)
}
pub fn input_source_name(device: &str) -> Result<&str> {
    let name = device
        .strip_prefix(INPUT_PREFIX)
        .ok_or_else(|| anyhow::anyhow!("Select a native Pulse input"))?;
    ensure!(
        !name.is_empty() && !name.contains('\0'),
        "Invalid Pulse input identity"
    );
    Ok(name)
}
pub struct PulseCapture {
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}
impl PulseCapture {
    pub fn start(
        device: &str,
        consume: impl FnMut(&[f32]) + Send + 'static,
        error: impl FnMut(String) + Send + 'static,
    ) -> Result<Self> {
        Self::start_with_latency_mode(device, MonitorLatencyMode::AdjustSource, consume, error)
    }
    /// Exact, non-monitor native input. Never resolves to the default mic.
    pub fn start_input_timed(
        device: &str,
        consume: impl FnMut(&[f32], Option<Instant>) + Send + 'static,
        error: impl FnMut(String) + Send + 'static,
    ) -> Result<Self> {
        Self::start_inner(
            device,
            MonitorLatencyMode::AdjustSource,
            true,
            true,
            consume,
            error,
        )
    }
    /// Timed live capture. Missing timing is passed explicitly, never guessed.
    pub fn start_timed(
        device: &str,
        consume: impl FnMut(&[f32], Option<Instant>) + Send + 'static,
        error: impl FnMut(String) + Send + 'static,
    ) -> Result<Self> {
        Self::start_inner(
            device,
            MonitorLatencyMode::AdjustSource,
            true,
            false,
            consume,
            error,
        )
    }
    /// Explicit policy for controlled comparisons. Source adjustment can affect
    /// a shared source; A/B tests must use a dedicated private server.
    pub fn start_with_latency_mode(
        device: &str,
        mode: MonitorLatencyMode,
        mut consume: impl FnMut(&[f32]) + Send + 'static,
        error: impl FnMut(String) + Send + 'static,
    ) -> Result<Self> {
        Self::start_inner(
            device,
            mode,
            false,
            false,
            move |data, _| consume(data),
            error,
        )
    }
    fn start_inner(
        device: &str,
        mode: MonitorLatencyMode,
        timed: bool,
        input: bool,
        mut consume: impl FnMut(&[f32], Option<Instant>) + Send + 'static,
        mut error: impl FnMut(String) + Send + 'static,
    ) -> Result<Self> {
        let source = if input {
            input_source_name(device)?.to_owned()
        } else {
            source_name(device)?.to_owned()
        };
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let thread = thread::Builder::new()
            .name("meetily-pulse-monitor".into())
            .spawn(move || {
                let setup = (|| {
                    let mut session = Session::connect(&stopped)?;
                    session.record(&source, &stopped, mode, input)?;
                    Ok::<_, anyhow::Error>(session)
                })();
                match setup {
                    Err(e) => {
                        let _ = ready_tx.send(Err(e.to_string()));
                    }
                    Ok(session) => {
                        if ready_tx.send(Ok(())).is_err() {
                            return;
                        }
                        let mut checked = Instant::now();
                        while !stopped.load(Ordering::Acquire) {
                            // PipeWire may leave a removed monitor stream READY
                            // but suspended indefinitely. Check source existence
                            // rather than treating silent callbacks as healthy.
                            if checked.elapsed() >= Duration::from_secs(1) {
                                let exists = session.source_names(&stopped, input).map(|names| names.iter().any(|n| n == &source));
                                match exists {
                                    Ok(true) => checked = Instant::now(),
                                    Ok(false) => { error("Selected Pulse monitor disappeared; reconnect explicitly".into()); break; }
                                    Err(e) => { if !stopped.load(Ordering::Acquire) { error(e.to_string()); } break; }
                                }
                            }
                            if let Err(e) = session.read(&stopped, timed, &mut consume) {
                                error(e.to_string());
                                break;
                            }
                            thread::sleep(Duration::from_millis(5));
                        }
                    }
                }
            })?;
        let capture = Self {
            stop,
            thread: Some(thread),
        };
        match ready_rx.recv_timeout(LIMIT * 3 + Duration::from_secs(1)) {
            Ok(Ok(())) => Ok(capture),
            Ok(Err(e)) => bail!(e),
            Err(_) => bail!("Pulse monitor startup timed out"),
        }
    }
}
impl Drop for PulseCapture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_input_identity_never_accepts_default_or_monitor_prefix() {
        assert!(input_source_name("default").is_err());
        assert!(input_source_name("pulse-input:").is_err());
        assert!(input_source_name("pulse-input:a\0b").is_err());
        assert!(input_source_name("pulse-monitor:fixture.monitor").is_err());
        assert_eq!(
            input_source_name("pulse-input:fixture_native_mic").unwrap(),
            "fixture_native_mic"
        );
    }
    #[test]
    fn identity_does_not_accept_default_or_microphone_names() {
        assert!(source_name("default").is_err());
        assert!(source_name("pulse-monitor:").is_err());
        assert!(source_name("pulse-monitor:x\0y").is_err());
        assert_eq!(
            source_name("pulse-monitor:dedicated.monitor").unwrap(),
            "dedicated.monitor"
        );
    }
    #[test]
    fn latency_policy_keeps_no_move_and_only_adjusts_when_explicit() {
        assert_eq!(MonitorLatencyMode::FragmentOnly.flags(), 0x0200);
        assert_eq!(MonitorLatencyMode::AdjustSource.flags(), 0x2200);
    }
    #[test]
    fn monitor_fragment_is_100ms_without_changing_other_buffer_policy() {
        let attr = monitor_buffer_attr();
        assert_eq!(attr.fragsize, 19_200); // mono float32, 48kHz
        assert_eq!(attr.fragsize % 4, 0);
        assert_eq!(attr.maxlength, u32::MAX);
        assert_eq!(attr.tlength, u32::MAX);
        assert_eq!(attr.prebuf, u32::MAX);
        assert_eq!(attr.minreq, u32::MAX);
    }
    #[test]
    fn stable_abi_prefix_layout() {
        assert_eq!(std::mem::size_of::<BufferAttr>(), 20);
        assert_eq!(std::mem::align_of::<BufferAttr>(), 4);
        assert_eq!(std::mem::offset_of!(BufferAttr, fragsize), 16);
        assert_eq!(std::mem::size_of::<SampleSpec>(), 12);
        assert_eq!(std::mem::size_of::<ChannelMap>(), 132);
        assert_eq!(std::mem::size_of::<Volume>(), 132);
        #[cfg(target_pointer_width = "64")]
        {
            assert_eq!(std::mem::offset_of!(SourceInfo, monitor_of_sink), 308);
        }
    }
}
