// audio/transcription/mod.rs
//
// Transcription module: Provider abstraction, engine management, and worker pool.

pub mod provider;
pub mod qwen_provider;
pub mod qwen_setup;
pub mod qwen_model;
pub mod batch;
pub mod engine;
pub mod worker;

// Re-export commonly used types
pub use provider::{TranscriptionError, TranscriptionProvider, TranscriptResult};
pub use engine::{
    TranscriptionEngine,
    validate_transcription_model_ready,
    get_or_init_transcription_engine
};
pub(crate) use worker::start_transcription_task;
pub use worker::{reset_speech_detected_flag, TranscriptUpdate};
