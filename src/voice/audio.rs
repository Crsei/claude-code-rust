//! Audio-capture backend abstraction (issue #13).
//!
//! The TS reference implementation (claude-code-bun) uses a native
//! cpal-backed module on macOS/Linux/Windows with SoX `rec` and ALSA
//! `arecord` as Linux fallbacks. For the Rust port, we wrap everything
//! behind a trait so those platform backends can land as follow-up PRs
//! without churning the command/controller layer.
//!
//! This module currently ships [`NullAudioBackend`] — an honest "not
//! implemented on this platform" stub that lets `/voice` gate correctly
//! and the UI state machine behave. A future backend module can
//! implement [`AudioCaptureBackend`] against cpal / SoX / arecord and
//! the rest of the voice subsystem will pick it up unchanged.

use std::sync::Arc;

use parking_lot::Mutex;
use tokio::sync::mpsc::UnboundedReceiver;

/// Public handle returned by [`AudioCaptureBackend::start`]. The caller
/// drains `audio` to forward PCM frames to the STT client, and invokes
/// [`RecordingHandle::stop`] when push-to-talk is released.
pub struct RecordingHandle {
    pub audio: UnboundedReceiver<Vec<u8>>,
    /// Cancel flag — flipped by [`Self::stop`] so the backend can tear
    /// down its capture thread.
    stopped: Arc<Mutex<bool>>,
}

impl std::fmt::Debug for RecordingHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RecordingHandle")
            .field("stopped", &*self.stopped.lock())
            .finish_non_exhaustive()
    }
}

impl RecordingHandle {
    pub fn new(audio: UnboundedReceiver<Vec<u8>>, stopped: Arc<Mutex<bool>>) -> Self {
        Self { audio, stopped }
    }

    /// Signal the backend to stop. Backends should poll this flag on
    /// their capture thread and drain / close the sender cleanly.
    pub fn stop(&self) {
        *self.stopped.lock() = true;
    }
}

impl Drop for RecordingHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Trait every audio backend implements.
///
/// Backends return PCM frames as `Vec<u8>` chunks (16 kHz mono s16le —
/// the format Anthropic's `voice_stream` expects). A real cpal backend
/// will resample / rechannel to this format before forwarding.
pub trait AudioCaptureBackend: Send + Sync {
    /// Short human label for diagnostics (`/voice status`).
    fn name(&self) -> &'static str;

    /// Probe whether the backend can actually start a recording. Called
    /// before `start()` so `/voice` can surface a specific error.
    fn is_available(&self) -> Result<(), AudioUnavailable>;

    /// Begin capturing audio. Returns a handle whose receiver yields
    /// PCM chunks. `Err` variants should match the same taxonomy as
    /// [`AudioUnavailable`].
    fn start(&self) -> Result<RecordingHandle, AudioUnavailable>;
}

/// Why an audio backend refused to record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioUnavailable {
    /// Backend is a stub — no implementation yet on this platform.
    NotImplemented(String),
    /// Backend or its native library is missing.
    ToolMissing(String),
    /// Microphone permission denied / no device.
    PermissionDenied(String),
    /// Remote environment (SSH, Homespace, WSL1) — no local mic.
    NoLocalAudio(String),
    /// Anything else with a one-line reason.
    Other(String),
}

impl AudioUnavailable {
    pub fn reason(&self) -> &str {
        match self {
            AudioUnavailable::NotImplemented(s)
            | AudioUnavailable::ToolMissing(s)
            | AudioUnavailable::PermissionDenied(s)
            | AudioUnavailable::NoLocalAudio(s)
            | AudioUnavailable::Other(s) => s.as_str(),
        }
    }
}

// ---------------------------------------------------------------------------
// NullAudioBackend — ships today; honest "not yet implemented" stub.
// ---------------------------------------------------------------------------

/// The default backend: always reports unavailable with a clear reason.
///
/// Used by `/voice` when no real backend has been compiled in. Because
/// the controller + command layer honour [`AudioUnavailable`], wiring a
/// real cpal backend later is a localized change.
pub struct NullAudioBackend {
    /// Reason surfaced through `is_available()` / `start()`. Useful for
    /// tests and future backends that want to return a different
    /// explanation without swapping the type.
    reason: String,
}

impl NullAudioBackend {
    pub fn new() -> Self {
        Self {
            reason: default_reason(),
        }
    }

    /// Construct a stub that reports the supplied reason — used by tests
    /// to exercise the different `AudioUnavailable` branches.
    #[cfg(test)]
    pub fn with_reason(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}

impl Default for NullAudioBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioCaptureBackend for NullAudioBackend {
    fn name(&self) -> &'static str {
        "null"
    }

    fn is_available(&self) -> Result<(), AudioUnavailable> {
        Err(AudioUnavailable::NotImplemented(self.reason.clone()))
    }

    fn start(&self) -> Result<RecordingHandle, AudioUnavailable> {
        Err(AudioUnavailable::NotImplemented(self.reason.clone()))
    }
}

fn default_reason() -> String {
    "Voice capture is not implemented in this build of cc-rust. \
     Native cpal + SoX / arecord backends land in a follow-up (issue #13)."
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_backend_reports_not_implemented_by_default() {
        let b = NullAudioBackend::new();
        assert_eq!(b.name(), "null");
        let err = b.is_available().unwrap_err();
        assert!(matches!(err, AudioUnavailable::NotImplemented(_)));
        let reason = err.reason().to_string();
        assert!(reason.contains("not implemented"), "{}", reason);
    }

    #[test]
    fn null_backend_start_returns_same_reason_as_is_available() {
        let b = NullAudioBackend::new();
        let a = b.is_available().unwrap_err();
        let s = b.start().unwrap_err();
        assert_eq!(a, s);
    }

    #[test]
    fn recording_handle_stop_flips_cancellation_flag() {
        let (_tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let flag = Arc::new(Mutex::new(false));
        let h = RecordingHandle::new(rx, Arc::clone(&flag));
        h.stop();
        assert!(*flag.lock());
    }

    #[test]
    fn dropping_recording_handle_signals_stop() {
        let (_tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let flag = Arc::new(Mutex::new(false));
        {
            let _h = RecordingHandle::new(rx, Arc::clone(&flag));
        }
        assert!(*flag.lock());
    }
}
