//! Speech-to-text client abstraction (issue #13).
//!
//! The reference implementation (claude-code-bun) uses Anthropic's
//! private `voice_stream` WebSocket — a claude.ai OAuth-gated endpoint
//! that ships PCM frames in and emits JSON control + text frames back.
//! Because the endpoint is unstable / undocumented and requires OAuth,
//! the Rust port keeps it behind a trait and ships a [`NullTranscriptionClient`]
//! for now. A follow-up PR can add a `VoiceStreamClient` built on
//! `tokio-tungstenite` once we're happy with the gating UX.
//!
//! Why abstract now instead of inlining later? So the controller's state
//! machine — which is the user-visible part of this ticket — can be
//! tested end-to-end today via a canned test client (see the
//! `tests::echo_client` helper).

use async_trait::async_trait;

use super::audio::RecordingHandle;

/// Result of running a full record-and-transcribe session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscriptionResult {
    /// Text to insert into the composer, trimmed.
    pub text: String,
    /// Language code actually sent to the STT service.
    pub language: String,
}

/// Every STT client implements this. Implementations drain
/// `handle.audio` for PCM frames and return a final transcription once
/// the handle closes (push-to-talk release).
#[async_trait]
pub trait TranscriptionClient: Send + Sync {
    /// Short label (`voice_stream`, `null`, etc.).
    fn name(&self) -> &'static str;

    /// Check whether the client can actually transcribe. Called before
    /// a recording session so `/voice` can surface a specific error.
    fn is_available(&self) -> Result<(), SttUnavailable>;

    /// Drive a full record-and-transcribe exchange. Implementations
    /// stream audio from `handle.audio` until the sender half closes,
    /// then return the final text.
    async fn transcribe(
        &self,
        handle: RecordingHandle,
        language: &str,
    ) -> Result<TranscriptionResult, SttError>;
}

/// Reasons the STT client cannot run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SttUnavailable {
    /// Endpoint not wired up in this build.
    NotImplemented(String),
    /// User isn't logged in with a supported auth.
    AuthRequired(String),
    /// Feature gate / kill switch disabled voice mode.
    Disabled(String),
    /// Anything else.
    Other(String),
}

impl SttUnavailable {
    pub fn reason(&self) -> &str {
        match self {
            SttUnavailable::NotImplemented(s)
            | SttUnavailable::AuthRequired(s)
            | SttUnavailable::Disabled(s)
            | SttUnavailable::Other(s) => s.as_str(),
        }
    }
}

/// Runtime errors surfaced from `transcribe()`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SttError {
    /// Upstream transport failed (socket reset, timeout, 5xx).
    Transport(String),
    /// Server returned an error response.
    Server(String),
    /// Client-side issue (bad format, cancelled).
    Client(String),
}

impl SttError {
    pub fn reason(&self) -> &str {
        match self {
            SttError::Transport(s) | SttError::Server(s) | SttError::Client(s) => s.as_str(),
        }
    }
}

// ---------------------------------------------------------------------------
// NullTranscriptionClient — placeholder until a real client lands.
// ---------------------------------------------------------------------------

/// Default client: reports unavailable with a clear reason.
pub struct NullTranscriptionClient {
    reason: String,
}

impl NullTranscriptionClient {
    pub fn new() -> Self {
        Self {
            reason: "Voice transcription is not wired up in this build. \
                     The Anthropic voice_stream WebSocket client lands in a \
                     follow-up (issue #13). Use /voice to toggle the flag \
                     once a backend is available."
                .to_string(),
        }
    }

    #[cfg(test)]
    pub fn with_reason(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}

impl Default for NullTranscriptionClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TranscriptionClient for NullTranscriptionClient {
    fn name(&self) -> &'static str {
        "null"
    }

    fn is_available(&self) -> Result<(), SttUnavailable> {
        Err(SttUnavailable::NotImplemented(self.reason.clone()))
    }

    async fn transcribe(
        &self,
        _handle: RecordingHandle,
        _language: &str,
    ) -> Result<TranscriptionResult, SttError> {
        Err(SttError::Client(self.reason.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voice::audio::AudioCaptureBackend;
    use crate::voice::audio::NullAudioBackend;

    /// Canned client used by the controller tests: echoes a fixed
    /// transcription back regardless of input audio. Demonstrates the
    /// trait is satisfiable by a non-trivial implementation.
    pub struct EchoClient {
        pub text: String,
    }

    #[async_trait]
    impl TranscriptionClient for EchoClient {
        fn name(&self) -> &'static str {
            "echo"
        }
        fn is_available(&self) -> Result<(), SttUnavailable> {
            Ok(())
        }
        async fn transcribe(
            &self,
            mut handle: RecordingHandle,
            language: &str,
        ) -> Result<TranscriptionResult, SttError> {
            // Drain any audio frames the test sent, just to exercise the
            // receiver path.
            while (handle.audio.recv().await).is_some() {}
            Ok(TranscriptionResult {
                text: self.text.clone(),
                language: language.to_string(),
            })
        }
    }

    #[test]
    fn null_client_reports_not_implemented() {
        let c = NullTranscriptionClient::new();
        assert_eq!(c.name(), "null");
        assert!(matches!(
            c.is_available().unwrap_err(),
            SttUnavailable::NotImplemented(_)
        ));
    }

    #[tokio::test]
    async fn null_client_transcribe_returns_client_error() {
        let c = NullTranscriptionClient::new();
        let b = NullAudioBackend::with_reason("stub");
        // We can't start a NullAudioBackend, so build a bare handle by
        // hand to exercise the async path.
        let (_tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let stopped = std::sync::Arc::new(parking_lot::Mutex::new(false));
        let h = RecordingHandle::new(rx, stopped);
        let r = c.transcribe(h, "en").await;
        assert!(matches!(r.unwrap_err(), SttError::Client(_)));
        // Keep `b` alive until the end of the test so the compiler
        // doesn't complain about an unused variable while still
        // documenting that a caller would usually pair the two.
        let _ = b.name();
    }
}
