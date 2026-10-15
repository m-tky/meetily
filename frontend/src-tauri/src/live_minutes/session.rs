//! Stable session binding survives Stop taking the global RecordingManager.
//! Keeping a local evidence handle is NOT consent to perform any model request.
use crate::audio::recording_saver::TranscriptSink;
use serde::Serialize;
use std::sync::{LazyLock, Mutex};

#[derive(Clone, Debug, Serialize)]
pub struct SessionInfo {
    pub id: String,
    pub ended: bool,
    pub transcription_drained: bool,
}
#[derive(Clone)]
pub(super) struct Session {
    pub info: SessionInfo,
    pub sink: TranscriptSink,
}
#[derive(Default)]
pub(crate) struct Store(Mutex<Option<Session>>);
impl Store {
    pub(crate) fn bind(&self, sink: TranscriptSink) {
        *self.0.lock().unwrap() = Some(Session {
            info: SessionInfo {
                id: uuid::Uuid::new_v4().to_string(),
                ended: false,
                transcription_drained: false,
            },
            sink,
        });
    }
    pub(crate) fn finish(&self, sink: &TranscriptSink, drained: bool) -> Option<SessionInfo> {
        let mut state = self.0.lock().unwrap();
        let current = state.as_mut()?;
        if !current.sink.same_session(sink) {
            return None;
        }
        current.info.ended = true;
        current.info.transcription_drained = drained;
        Some(current.info.clone())
    }
    pub(super) fn capture(&self, id: &str, final_pass: bool) -> Result<Session, String> {
        let state = self.0.lock().map_err(|_| "Minutes session lock failed")?;
        let current = state.as_ref().ok_or("No current meeting folder")?;
        if current.info.id != id {
            return Err("Recording changed; minutes session mismatch".into());
        }
        if final_pass && (!current.info.ended || !current.info.transcription_drained) {
            return Err(
                "Final minutes unavailable: transcription tail did not finish successfully".into(),
            );
        }
        Ok(current.clone())
    }
    pub(super) fn is_current(&self, id: &str) -> bool {
        self.0
            .lock()
            .ok()
            .and_then(|s| s.as_ref().map(|s| s.info.id == id))
            .unwrap_or(false)
    }
    pub(crate) fn info(&self) -> Option<SessionInfo> {
        self.0
            .lock()
            .ok()
            .and_then(|s| s.as_ref().map(|s| s.info.clone()))
    }
}
pub(crate) static SESSIONS: LazyLock<Store> = LazyLock::new(Store::default);

#[tauri::command]
pub fn api_get_live_minutes_session() -> Option<SessionInfo> {
    SESSIONS.info()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::recording_saver::{RecordingSaver, TranscriptSegment};
    fn segment(text: &str) -> TranscriptSegment {
        TranscriptSegment {
            id: "s1".into(),
            text: text.into(),
            audio_start_time: 0.0,
            audio_end_time: 1.0,
            duration: 1.0,
            display_time: "[00:00]".into(),
            confidence: 0.0,
            sequence_id: 1,
            speaker: None,
        }
    }
    #[test]
    fn stop_retains_tail_and_new_session_rejects_stale_requests() {
        let store = Store::default();
        let saver = RecordingSaver::new();
        let sink = saver.transcript_sink();
        store.bind(sink.clone());
        let id = store.info().unwrap().id;
        assert!(store.capture(&id, true).is_err());
        drop(saver);
        sink.add(segment("停止後の末尾")).unwrap();
        store.finish(&sink, true).unwrap();
        assert_eq!(
            store.capture(&id, true).unwrap().sink.snapshot().unwrap()[0].text,
            "停止後の末尾"
        );
        let next = RecordingSaver::new();
        store.bind(next.transcript_sink());
        assert!(!store.is_current(&id));
        assert!(store.capture(&id, true).is_err());
        assert!(store.finish(&sink, true).is_none());
        assert!(!store.info().unwrap().ended);
    }
    #[test]
    fn incomplete_stop_cannot_start_final_model_request() {
        let store = Store::default();
        let saver = RecordingSaver::new();
        let sink = saver.transcript_sink();
        store.bind(sink.clone());
        let id = store.info().unwrap().id;
        store.finish(&sink, false);
        assert!(store.capture(&id, true).is_err());
        assert!(store.capture(&id, false).is_ok()); // manual available-evidence update, not final.
    }
}
