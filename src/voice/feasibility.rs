//! Feasibility checks for voice dictation (issue #13).
//!
//! `/voice` and the push-to-talk keybinding both need to know, before
//! opening a microphone, whether voice mode is allowed in this
//! environment. The claude-code-bun reference gates on:
//!
//! 1. Anthropic OAuth is the active auth (API keys / Bedrock / Vertex /
//!    Foundry don't have the `voice_stream` endpoint).
//! 2. A growthbook kill-switch (`tengu_amber_quartz_disabled`).
//! 3. Not running in a remote / SSH environment.
//! 4. Native audio module or SoX / arecord is present.
//! 5. OS microphone permission is granted.
//!
//! The cc-rust port only needs (1), (3) and (4) at the feasibility-check
//! layer — (5) is delegated to the capture backend's probe, (2) is
//! absent (no growthbook). We return a [`Feasibility`] enum so the
//! command handler can print a specific, actionable error per reason.

use crate::auth::AuthMethod;

use super::audio::{AudioCaptureBackend, AudioUnavailable};
use super::stt::{SttUnavailable, TranscriptionClient};

/// Whether voice mode is usable right now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Feasibility {
    /// Ready: the user can flip `voiceEnabled` and use push-to-talk.
    Ready {
        /// Friendly backend label (e.g. `"null"`, `"cpal"`, `"voice_stream"`).
        backend: String,
        /// STT client label.
        client: String,
    },
    /// Not usable for a specific reason.
    Blocked(FeasibilityReason),
}

impl Feasibility {
    pub fn is_ready(&self) -> bool {
        matches!(self, Feasibility::Ready { .. })
    }
    pub fn reason(&self) -> Option<&FeasibilityReason> {
        match self {
            Feasibility::Ready { .. } => None,
            Feasibility::Blocked(r) => Some(r),
        }
    }
}

/// Concrete blocker surfaced to the user by `/voice`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeasibilityReason {
    /// Auth layer can't serve `voice_stream` — API keys, Bedrock, etc.
    UnsupportedAuth {
        auth_label: String,
        message: String,
    },
    /// User isn't logged in at all.
    NotAuthenticated(String),
    /// Remote environment (SSH session, CC_RUST_REMOTE).
    RemoteEnvironment(String),
    /// Audio capture backend isn't available.
    AudioBackend(AudioUnavailable),
    /// STT endpoint isn't available.
    SttBackend(SttUnavailable),
}

impl FeasibilityReason {
    pub fn short(&self) -> &str {
        match self {
            FeasibilityReason::UnsupportedAuth { message, .. }
            | FeasibilityReason::NotAuthenticated(message)
            | FeasibilityReason::RemoteEnvironment(message) => message.as_str(),
            FeasibilityReason::AudioBackend(u) => u.reason(),
            FeasibilityReason::SttBackend(u) => u.reason(),
        }
    }
}

/// Check (in order) whether voice is supported right now. Returns the
/// first blocker we hit — callers don't need to distinguish cascaded
/// failures.
pub fn check_feasibility(
    auth: &AuthMethod,
    audio: &dyn AudioCaptureBackend,
    stt: &dyn TranscriptionClient,
) -> Feasibility {
    if let Some(reason) = auth_blocker(auth) {
        return Feasibility::Blocked(reason);
    }
    if let Some(reason) = remote_environment_blocker() {
        return Feasibility::Blocked(reason);
    }
    if let Err(e) = audio.is_available() {
        return Feasibility::Blocked(FeasibilityReason::AudioBackend(e));
    }
    if let Err(e) = stt.is_available() {
        return Feasibility::Blocked(FeasibilityReason::SttBackend(e));
    }
    Feasibility::Ready {
        backend: audio.name().to_string(),
        client: stt.name().to_string(),
    }
}

/// Reject auth methods that can't reach the Anthropic `voice_stream`
/// endpoint. Only Claude.ai OAuth is supported upstream.
pub fn auth_blocker(auth: &AuthMethod) -> Option<FeasibilityReason> {
    match auth {
        AuthMethod::None => Some(FeasibilityReason::NotAuthenticated(
            "Voice mode requires a Claude.ai account. Run `/login` to sign in."
                .to_string(),
        )),
        AuthMethod::ApiKey(_) => Some(FeasibilityReason::UnsupportedAuth {
            auth_label: "api_key".into(),
            message: "Voice mode requires Claude.ai OAuth. API keys, Bedrock, Vertex, \
                      and Foundry cannot reach the voice_stream endpoint. \
                      Run `/login` to switch to OAuth."
                .into(),
        }),
        AuthMethod::ExternalToken(_) => Some(FeasibilityReason::UnsupportedAuth {
            auth_label: "external_token".into(),
            message: "Voice mode requires Claude.ai OAuth. External bearer tokens \
                      (ANTHROPIC_AUTH_TOKEN) aren't supported by voice_stream."
                .into(),
        }),
        AuthMethod::OAuthToken { method, .. } => {
            // Only Claude.ai OAuth works; Console and OpenAI Codex OAuth do not.
            if method == "claude_ai" {
                None
            } else {
                Some(FeasibilityReason::UnsupportedAuth {
                    auth_label: method.clone(),
                    message: format!(
                        "Voice mode requires Claude.ai OAuth. The active OAuth method \
                         ({}) does not include voice_stream.",
                        method
                    ),
                })
            }
        }
    }
}

/// Detect remote-run environments where a local microphone is unlikely.
/// Exact parity with claude-code-bun's `isRunningOnHomespace()` /
/// `CLAUDE_CODE_REMOTE` check.
fn remote_environment_blocker() -> Option<FeasibilityReason> {
    if truthy_env("CC_RUST_REMOTE") || truthy_env("CLAUDE_CODE_REMOTE") {
        return Some(FeasibilityReason::RemoteEnvironment(
            "Voice mode needs a local microphone, but CC_RUST_REMOTE is set — \
             this looks like a remote / Homespace session. Run cc-rust locally \
             to use voice."
                .to_string(),
        ));
    }
    // SSH connections set SSH_TTY / SSH_CONNECTION; most are headless and
    // have no audio device forwarded. Warn but don't block — users with
    // X11-forwarded audio may still work.
    None
}

fn truthy_env(key: &str) -> bool {
    match std::env::var(key) {
        Ok(v) => matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voice::audio::NullAudioBackend;
    use crate::voice::stt::NullTranscriptionClient;

    fn null_pair() -> (NullAudioBackend, NullTranscriptionClient) {
        (NullAudioBackend::new(), NullTranscriptionClient::new())
    }

    #[test]
    fn api_key_auth_is_rejected_with_switch_to_oauth_message() {
        let (a, s) = null_pair();
        let auth = AuthMethod::ApiKey("sk-foo".into());
        let f = check_feasibility(&auth, &a, &s);
        match f.reason().unwrap() {
            FeasibilityReason::UnsupportedAuth { auth_label, message } => {
                assert_eq!(auth_label, "api_key");
                assert!(message.contains("Claude.ai OAuth"));
            }
            other => panic!("expected UnsupportedAuth, got {:?}", other),
        }
    }

    #[test]
    fn none_auth_is_rejected_with_login_hint() {
        let (a, s) = null_pair();
        let auth = AuthMethod::None;
        let f = check_feasibility(&auth, &a, &s);
        assert!(matches!(
            f.reason().unwrap(),
            FeasibilityReason::NotAuthenticated(_)
        ));
    }

    #[test]
    fn claude_ai_oauth_passes_auth_gate() {
        let (a, s) = null_pair();
        let auth = AuthMethod::OAuthToken {
            access_token: "tok".into(),
            method: "claude_ai".into(),
        };
        let f = check_feasibility(&auth, &a, &s);
        // Audio null backend kicks in after auth passes.
        assert!(matches!(
            f.reason().unwrap(),
            FeasibilityReason::AudioBackend(AudioUnavailable::NotImplemented(_))
        ));
    }

    #[test]
    fn non_claude_ai_oauth_is_rejected() {
        let (a, s) = null_pair();
        let auth = AuthMethod::OAuthToken {
            access_token: "tok".into(),
            method: "console".into(),
        };
        let f = check_feasibility(&auth, &a, &s);
        match f.reason().unwrap() {
            FeasibilityReason::UnsupportedAuth { auth_label, .. } => {
                assert_eq!(auth_label, "console");
            }
            other => panic!("expected UnsupportedAuth, got {:?}", other),
        }
    }

    #[test]
    fn remote_env_var_blocks_even_with_good_auth() {
        // Serialize on a single thread so the env mutation doesn't race
        // another test. See also the guard comment below.
        std::env::set_var("CC_RUST_REMOTE", "1");
        let (a, s) = null_pair();
        let auth = AuthMethod::OAuthToken {
            access_token: "tok".into(),
            method: "claude_ai".into(),
        };
        let f = check_feasibility(&auth, &a, &s);
        let reason = f.reason().unwrap().clone();
        std::env::remove_var("CC_RUST_REMOTE");
        assert!(
            matches!(reason, FeasibilityReason::RemoteEnvironment(_)),
            "got {:?}",
            reason
        );
    }
}
