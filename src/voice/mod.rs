//! Voice dictation subsystem (issue #13).
//!
//! Implements the push-to-talk interaction described in
//! `docs/claude-code-configuration/voice-dictation.md`. The surface area
//! is intentionally small and trait-driven so platform-specific audio
//! capture and transcription can land in follow-up work without changing
//! the TUI / command layer.
//!
//! # Layout
//!
//! - [`controller`] — [`VoiceController`] + [`VoiceState`], the front-end
//!   state machine consumed by the TUI.
//! - [`audio`] — [`AudioCaptureBackend`] trait + `NullBackend` /
//!   environment-probe stub. Real cpal / SoX / arecord backends are a
//!   follow-up.
//! - [`stt`] — [`TranscriptionClient`] trait + `NullClient` stub. The
//!   real Anthropic `voice_stream` WebSocket client is a follow-up.
//! - [`language`] — `normalize_language_for_stt` maps a freeform
//!   `language` setting (BCP-47 / alias / display name) to a supported
//!   dictation language code, mirroring the TS helper.
//! - [`feasibility`] — gates the controller: auth, backend, recording
//!   environment, platform.
//!
//! # Acceptance criteria (issue #13)
//!
//! - `/voice` command exists → `commands/voice_cmd.rs`
//! - `voiceEnabled` config plumbed → already in
//!   `config/settings.rs`/`types/app_state.rs`; `/voice` flips it
//! - `voice:pushToTalk` action is bindable → registered in
//!   `keybindings/defaults.rs`
//! - Transcription output inserts at cursor → `App` handles the
//!   `AppAction::InsertText` produced by the controller
//! - Unsupported auth/backend gives a clear error →
//!   [`feasibility::check_feasibility`]
//! - `language` affects dictation → [`language::normalize_language_for_stt`]
//!
//! # Dead-code note
//!
//! Several types here are only consumed by downstream modules (the
//! `/voice` slash command, the TUI `App`, future cpal / voice_stream
//! backends). The module-level `#[allow(dead_code)]` keeps the public
//! surface visible without cluttering each item with its own allow.

#![allow(dead_code)]

pub mod audio;
pub mod controller;
pub mod feasibility;
pub mod language;
pub mod stt;

// Re-exports kept flat so command / TUI code can write
// `crate::voice::VoiceController` etc. Marked `pub` so the surface is
// documented; `#[allow(unused_imports)]` skips the warning while one or
// two items are still consumed only through their full path.
#[allow(unused_imports)]
pub use audio::{AudioCaptureBackend, NullAudioBackend, RecordingHandle};
#[allow(unused_imports)]
pub use controller::{VoiceController, VoiceEvent, VoiceState};
#[allow(unused_imports)]
pub use feasibility::{Feasibility, FeasibilityReason};
#[allow(unused_imports)]
pub use language::{normalize_language_for_stt, NormalizedLanguage};
#[allow(unused_imports)]
pub use stt::{NullTranscriptionClient, TranscriptionClient, TranscriptionResult};
