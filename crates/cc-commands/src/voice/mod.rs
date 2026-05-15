//! Voice dictation compatibility helpers used by `/voice`.

#![allow(dead_code)]

pub mod audio;
pub mod feasibility;
pub mod language;
pub mod stt;

#[allow(unused_imports)]
pub use audio::{AudioCaptureBackend, NullAudioBackend, RecordingHandle};
#[allow(unused_imports)]
pub use feasibility::{Feasibility, FeasibilityReason};
#[allow(unused_imports)]
pub use language::{normalize_language_for_stt, NormalizedLanguage};
#[allow(unused_imports)]
pub use stt::{NullTranscriptionClient, TranscriptionClient, TranscriptionResult};
