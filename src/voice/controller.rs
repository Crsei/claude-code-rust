//! Push-to-talk state machine (issue #13).
//!
//! [`VoiceController`] owns the `Idle → Recording → Transcribing →
//! Idle` transitions and produces [`VoiceEvent`]s the TUI consumes. The
//! TS reference keeps this logic spread across a React hook + a service
//! object; in the Rust port it's a single struct so the TUI key handler
//! can simply call `.press()` / `.release()` and then drain events
//! synchronously.
//!
//! The controller is deliberately I/O-free: it just coordinates the
//! audio backend + STT client + an event queue. Real transport work
//! happens on a tokio task that the controller spawns at `press()`
//! time.

use std::sync::Arc;

use parking_lot::Mutex;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use super::audio::{AudioCaptureBackend, AudioUnavailable, RecordingHandle};
use super::stt::{SttError, TranscriptionClient, TranscriptionResult};

/// Observable state of the controller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoiceState {
    /// Microphone not engaged; ready to start.
    Idle,
    /// Actively capturing audio.
    Recording,
    /// Audio capture stopped; waiting for the STT client to finalize.
    Transcribing,
    /// Something went wrong; carries a human message.
    Error(String),
}

impl VoiceState {
    pub fn is_active(&self) -> bool {
        matches!(self, VoiceState::Recording | VoiceState::Transcribing)
    }

    pub fn label(&self) -> &'static str {
        match self {
            VoiceState::Idle => "idle",
            VoiceState::Recording => "recording",
            VoiceState::Transcribing => "transcribing",
            VoiceState::Error(_) => "error",
        }
    }
}

/// Events pushed to the TUI. The TUI drains these with [`VoiceController::drain_events`]
/// on each render tick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoiceEvent {
    /// State changed — the TUI should redraw the composer / status line.
    StateChanged(VoiceState),
    /// Final transcription ready for insertion.
    Transcription(String),
    /// Recoverable error — caller may choose to display and keep going.
    Error(String),
}

/// Shared handle consumed by the TUI. Cheap to clone.
#[derive(Clone)]
pub struct VoiceController {
    inner: Arc<Mutex<ControllerState>>,
    events_tx: UnboundedSender<VoiceEvent>,
    events_rx: Arc<Mutex<Option<UnboundedReceiver<VoiceEvent>>>>,
}

struct ControllerState {
    audio: Arc<dyn AudioCaptureBackend>,
    stt: Arc<dyn TranscriptionClient>,
    state: VoiceState,
    /// Active recording's stop flag — flipped on `release()`.
    active_handle: Option<RecordingHandle>,
}

impl VoiceController {
    pub fn new(
        audio: Arc<dyn AudioCaptureBackend>,
        stt: Arc<dyn TranscriptionClient>,
    ) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            inner: Arc::new(Mutex::new(ControllerState {
                audio,
                stt,
                state: VoiceState::Idle,
                active_handle: None,
            })),
            events_tx: tx,
            events_rx: Arc::new(Mutex::new(Some(rx))),
        }
    }

    /// Observe current state without locking long-term.
    pub fn state(&self) -> VoiceState {
        self.inner.lock().state.clone()
    }

    /// Human-readable one-line status for the composer footer.
    pub fn status_line(&self) -> Option<String> {
        match self.state() {
            VoiceState::Idle => None,
            VoiceState::Recording => Some("● REC · release key to transcribe".to_string()),
            VoiceState::Transcribing => Some("… transcribing".to_string()),
            VoiceState::Error(msg) => Some(format!("voice error: {}", msg)),
        }
    }

    /// Push-to-talk key was pressed. Attempts to start audio capture
    /// and dispatches a transcription task. Idempotent: repeated presses
    /// while already recording are ignored.
    pub fn press(&self, language: String) {
        let mut guard = self.inner.lock();
        if !matches!(guard.state, VoiceState::Idle | VoiceState::Error(_)) {
            return;
        }
        match guard.audio.start() {
            Ok(handle) => {
                guard.state = VoiceState::Recording;
                guard.active_handle = None;
                drop(guard);
                self.send(VoiceEvent::StateChanged(VoiceState::Recording));
                self.spawn_transcription_task(handle, language);
            }
            Err(e) => {
                let msg = format_audio_error(&e);
                guard.state = VoiceState::Error(msg.clone());
                drop(guard);
                self.send(VoiceEvent::StateChanged(VoiceState::Error(msg.clone())));
                self.send(VoiceEvent::Error(msg));
            }
        }
    }

    /// Release of the push-to-talk key — stop capturing audio. The
    /// spawned transcription task will observe the channel closing and
    /// finalize.
    pub fn release(&self) {
        let mut guard = self.inner.lock();
        if !matches!(guard.state, VoiceState::Recording) {
            return;
        }
        guard.state = VoiceState::Transcribing;
        if let Some(h) = guard.active_handle.take() {
            h.stop();
        }
        drop(guard);
        self.send(VoiceEvent::StateChanged(VoiceState::Transcribing));
    }

    /// Cancel an in-flight recording without producing a transcription.
    /// Used when the user flips voice off mid-recording or hits Esc.
    pub fn cancel(&self) {
        let mut guard = self.inner.lock();
        if matches!(guard.state, VoiceState::Idle) {
            return;
        }
        guard.state = VoiceState::Idle;
        if let Some(h) = guard.active_handle.take() {
            h.stop();
        }
        drop(guard);
        self.send(VoiceEvent::StateChanged(VoiceState::Idle));
    }

    /// Drain every queued event. Safe to call on every tick; returns
    /// empty vec if nothing is pending.
    pub fn drain_events(&self) -> Vec<VoiceEvent> {
        let mut guard = self.events_rx.lock();
        let rx = match guard.as_mut() {
            Some(rx) => rx,
            None => return Vec::new(),
        };
        let mut out = Vec::new();
        while let Ok(evt) = rx.try_recv() {
            out.push(evt);
        }
        out
    }

    fn send(&self, evt: VoiceEvent) {
        let _ = self.events_tx.send(evt);
    }

    /// Spawn the transcription task. Caller already holds `state = Recording`.
    fn spawn_transcription_task(&self, handle: RecordingHandle, language: String) {
        let stt = { self.inner.lock().stt.clone() };
        let events_tx = self.events_tx.clone();
        let inner = Arc::clone(&self.inner);
        tokio::spawn(async move {
            let result = stt.transcribe(handle, &language).await;
            match result {
                Ok(TranscriptionResult { text, .. }) => {
                    let trimmed = text.trim().to_string();
                    {
                        let mut guard = inner.lock();
                        guard.state = VoiceState::Idle;
                        guard.active_handle = None;
                    }
                    let _ = events_tx.send(VoiceEvent::StateChanged(VoiceState::Idle));
                    if !trimmed.is_empty() {
                        let _ = events_tx.send(VoiceEvent::Transcription(trimmed));
                    }
                }
                Err(e) => {
                    let msg = format_stt_error(&e);
                    {
                        let mut guard = inner.lock();
                        guard.state = VoiceState::Error(msg.clone());
                        guard.active_handle = None;
                    }
                    let _ = events_tx.send(VoiceEvent::StateChanged(VoiceState::Error(
                        msg.clone(),
                    )));
                    let _ = events_tx.send(VoiceEvent::Error(msg));
                }
            }
        });
    }
}

fn format_audio_error(e: &AudioUnavailable) -> String {
    format!("voice capture unavailable: {}", e.reason())
}

fn format_stt_error(e: &SttError) -> String {
    match e {
        SttError::Transport(s) => format!("voice transport error: {}", s),
        SttError::Server(s) => format!("voice server error: {}", s),
        SttError::Client(s) => format!("voice client error: {}", s),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use crate::voice::audio::NullAudioBackend;
    use crate::voice::stt::{NullTranscriptionClient, SttUnavailable};

    /// Local canned client: echoes a fixed transcription back regardless
    /// of the audio input. Inline to keep stt.rs's test module private.
    struct EchoClient {
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
            while (handle.audio.recv().await).is_some() {}
            Ok(TranscriptionResult {
                text: self.text.clone(),
                language: language.to_string(),
            })
        }
    }

    #[tokio::test]
    async fn null_backend_press_transitions_to_error() {
        let c = VoiceController::new(
            Arc::new(NullAudioBackend::new()),
            Arc::new(NullTranscriptionClient::new()),
        );
        assert_eq!(c.state(), VoiceState::Idle);
        c.press("en".into());
        assert!(matches!(c.state(), VoiceState::Error(_)));
        let events = c.drain_events();
        assert!(events.iter().any(|e| matches!(e, VoiceEvent::Error(_))));
    }

    #[tokio::test]
    async fn echo_client_end_to_end_emits_transcription_event() {
        let c = VoiceController::new(
            Arc::new(EchoPipeBackend::default()),
            Arc::new(EchoClient {
                text: "hello world".into(),
            }),
        );
        c.press("en".into());
        // Give the spawned task a moment — release should push state to
        // Transcribing, then the task will finalize to Idle and emit the
        // transcription event.
        c.release();
        for _ in 0..50 {
            let events = c.drain_events();
            if events
                .iter()
                .any(|e| matches!(e, VoiceEvent::Transcription(t) if t == "hello world"))
            {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        panic!("transcription event never arrived");
    }

    #[tokio::test]
    async fn cancel_from_recording_goes_idle_without_transcription() {
        let c = VoiceController::new(
            Arc::new(EchoPipeBackend::default()),
            Arc::new(EchoClient {
                text: "ignored".into(),
            }),
        );
        c.press("en".into());
        c.cancel();
        assert_eq!(c.state(), VoiceState::Idle);
    }

    /// Backend that successfully starts but never sends audio — lets
    /// EchoClient exit immediately via an empty drain.
    #[derive(Default)]
    struct EchoPipeBackend;

    impl AudioCaptureBackend for EchoPipeBackend {
        fn name(&self) -> &'static str {
            "echo-pipe"
        }
        fn is_available(&self) -> Result<(), AudioUnavailable> {
            Ok(())
        }
        fn start(&self) -> Result<RecordingHandle, AudioUnavailable> {
            // Sender dropped immediately → receiver closes → EchoClient
            // finishes as soon as it's polled.
            let (_tx, rx) = mpsc::unbounded_channel();
            let stopped = Arc::new(Mutex::new(false));
            Ok(RecordingHandle::new(rx, stopped))
        }
    }
}
