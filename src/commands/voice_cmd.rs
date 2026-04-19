//! `/voice` slash command — toggle voice dictation and run pre-flight
//! checks (issue #13).
//!
//! Mirrors the behaviour of `claude-code-bun/src/commands/voice/voice.ts`:
//!
//! - `/voice` / `/voice status` — show whether voice is enabled + any
//!   feasibility blockers (auth, audio backend, STT client).
//! - `/voice on` / `/voice enable` — flip `voiceEnabled = true` and
//!   persist to user settings; fails fast with a specific reason when
//!   the environment is hostile (API key auth, no mic, remote session).
//! - `/voice off` / `/voice disable` — flip to false. No pre-flight
//!   needed.
//! - `/voice toggle` — equivalent to on/off depending on the current
//!   state.
//! - `/voice diagnose` — long-form environment dump (auth method,
//!   backend label, language mapping) for filing bug reports.
//!
//! Persisted changes go to `~/.cc-rust/settings.json` (user scope) via
//! the existing `settings::write_user_settings` helper. The in-memory
//! `AppState.settings.voice_enabled` is also patched so the change
//! takes effect within the current session.

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};
use crate::auth::{self, AuthMethod};
use crate::config::settings::{self, RawSettings};
use crate::voice::audio::NullAudioBackend;
use crate::voice::feasibility::{check_feasibility, Feasibility, FeasibilityReason};
use crate::voice::language::normalize_language_for_stt;
use crate::voice::stt::NullTranscriptionClient;

pub struct VoiceHandler;

#[async_trait]
impl CommandHandler for VoiceHandler {
    async fn execute(&self, args: &str, ctx: &mut CommandContext) -> Result<CommandResult> {
        let sub = args
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        let output = match sub.as_str() {
            "" | "status" | "show" => render_status(ctx),
            "on" | "enable" => toggle(true, ctx).await,
            "off" | "disable" => toggle(false, ctx).await,
            "toggle" => {
                let next = !ctx.app_state.settings.voice_enabled.unwrap_or(false);
                toggle(next, ctx).await
            }
            "diagnose" | "doctor" => render_diagnose(ctx),
            other => usage(other),
        };
        Ok(CommandResult::Output(output))
    }
}

fn usage(other: &str) -> String {
    format!(
        "Unknown /voice subcommand '{}'.\n\nUsage:\n  \
         /voice                 — show state + pre-flight check\n  \
         /voice on | enable     — enable voice dictation\n  \
         /voice off | disable   — disable voice dictation\n  \
         /voice toggle          — flip the enabled flag\n  \
         /voice diagnose        — full environment dump",
        other
    )
}

fn render_status(ctx: &CommandContext) -> String {
    let enabled = ctx.app_state.settings.voice_enabled.unwrap_or(false);
    let lang = normalize_language_for_stt(ctx.app_state.settings.language.as_deref());
    let feas = current_feasibility();

    let mut out = String::new();
    out.push_str("Voice dictation\n");
    out.push_str("───────────────\n");
    out.push_str(&format!(
        "  voiceEnabled:      {}\n",
        if enabled { "true" } else { "false" }
    ));
    out.push_str(&format!("  dictation lang:    {}", lang.code));
    if let Some(raw) = &lang.fell_back_from {
        out.push_str(&format!(
            " (falling back from \"{}\" — not supported by voice_stream)",
            raw
        ));
    }
    out.push('\n');
    match &feas {
        Feasibility::Ready { backend, client } => {
            out.push_str(&format!("  audio backend:     {}\n", backend));
            out.push_str(&format!("  stt client:        {}\n", client));
            out.push_str("  feasibility:       ready\n");
        }
        Feasibility::Blocked(reason) => {
            out.push_str(&format!(
                "  feasibility:       blocked ({})\n",
                short_reason_code(reason)
            ));
            out.push_str(&format!("  reason:            {}\n", reason.short()));
        }
    }

    out.push_str(
        "\nPush-to-talk: hold `Ctrl+Space` in the chat input to record; release to transcribe.\n",
    );
    out.push_str("Config path: ");
    out.push_str(&settings::user_settings_path().display().to_string());
    out.push('\n');
    out
}

fn render_diagnose(ctx: &CommandContext) -> String {
    let mut out = render_status(ctx);
    out.push('\n');
    out.push_str("Environment\n");
    out.push_str("───────────\n");
    let auth = auth::resolve_auth();
    out.push_str(&format!("  auth method:       {}\n", auth_label(&auth)));
    out.push_str(&format!(
        "  CC_RUST_REMOTE:    {}\n",
        std::env::var("CC_RUST_REMOTE").unwrap_or_else(|_| "(unset)".into())
    ));
    out.push_str(&format!(
        "  CLAUDE_CODE_REMOTE: {}\n",
        std::env::var("CLAUDE_CODE_REMOTE").unwrap_or_else(|_| "(unset)".into())
    ));
    out.push_str(&format!(
        "  language setting:  {}\n",
        ctx.app_state
            .settings
            .language
            .as_deref()
            .unwrap_or("(unset)")
    ));
    out.push('\n');
    out.push_str(
        "Note: native audio capture + voice_stream WebSocket transcription are stubbed in this \
         build of cc-rust. /voice is wired end-to-end (config + command + keybinding + UI), but \
         pushing Ctrl+Space currently produces a feasibility error. A cpal-based capture backend \
         + voice_stream client are tracked as follow-ups to issue #13.\n",
    );
    out
}

async fn toggle(target: bool, ctx: &mut CommandContext) -> String {
    // Turning OFF: no feasibility check required.
    if !target {
        return match persist(false) {
            Ok(path) => {
                ctx.app_state.settings.voice_enabled = Some(false);
                format!(
                    "Voice dictation disabled.\n→ persisted to {}",
                    path.display()
                )
            }
            Err(e) => format!("Failed to persist voiceEnabled=false: {}", e),
        };
    }

    // Turning ON: run the feasibility gate first.
    match current_feasibility() {
        Feasibility::Blocked(reason) => format_blocked(&reason),
        Feasibility::Ready { .. } => match persist(true) {
            Ok(path) => {
                ctx.app_state.settings.voice_enabled = Some(true);
                let lang = normalize_language_for_stt(ctx.app_state.settings.language.as_deref());
                let mut out = format!(
                    "Voice dictation enabled.\n→ persisted to {}",
                    path.display()
                );
                if let Some(raw) = lang.fell_back_from {
                    out.push_str(&format!(
                        "\nNote: \"{}\" is not a supported dictation language; falling back to `en`. \
                         Change it via /config set language <code>.",
                        raw
                    ));
                } else {
                    out.push_str(&format!(
                        "\nDictation language: {} (from /config language).",
                        lang.code
                    ));
                }
                out.push_str("\nHold Ctrl+Space to record.");
                out
            }
            Err(e) => format!("Failed to persist voiceEnabled=true: {}", e),
        },
    }
}

fn format_blocked(reason: &FeasibilityReason) -> String {
    let mut out = String::from("Voice mode is not available.\n\n");
    out.push_str(&format!("Reason: {}\n", reason.short()));
    match reason {
        FeasibilityReason::UnsupportedAuth { .. } | FeasibilityReason::NotAuthenticated(_) => {
            out.push_str("Hint: run `/login` and select the Claude.ai OAuth option.\n");
        }
        FeasibilityReason::RemoteEnvironment(_) => {
            out.push_str(
                "Hint: unset CC_RUST_REMOTE / CLAUDE_CODE_REMOTE, or run cc-rust locally.\n",
            );
        }
        FeasibilityReason::AudioBackend(_) => {
            out.push_str(
                "Hint: a capture backend is not compiled in. Build cc-rust with a cpal \
                 backend, or install SoX / arecord on Linux.\n",
            );
        }
        FeasibilityReason::SttBackend(_) => {
            out.push_str(
                "Hint: the STT client is not wired up. Track the voice_stream follow-up in issue #13.\n",
            );
        }
    }
    out
}

/// Produce a [`Feasibility`] snapshot using the live auth + the null
/// backends. Shared so both `render_status` and `toggle(true)` see the
/// same view.
pub fn current_feasibility() -> Feasibility {
    let auth = auth::resolve_auth();
    let audio = NullAudioBackend::new();
    let stt = NullTranscriptionClient::new();
    check_feasibility(&auth, &audio, &stt)
}

fn short_reason_code(r: &FeasibilityReason) -> &'static str {
    match r {
        FeasibilityReason::UnsupportedAuth { .. } => "unsupported_auth",
        FeasibilityReason::NotAuthenticated(_) => "not_authenticated",
        FeasibilityReason::RemoteEnvironment(_) => "remote_environment",
        FeasibilityReason::AudioBackend(_) => "audio_backend",
        FeasibilityReason::SttBackend(_) => "stt_backend",
    }
}

fn auth_label(a: &AuthMethod) -> &'static str {
    match a {
        AuthMethod::None => "none",
        AuthMethod::ApiKey(_) => "api_key",
        AuthMethod::ExternalToken(_) => "external_token",
        AuthMethod::OAuthToken { method, .. } => match method.as_str() {
            "claude_ai" => "oauth.claude_ai",
            "console" => "oauth.console",
            "openai_codex" => "oauth.openai_codex",
            _ => "oauth.other",
        },
    }
}

/// Read / patch / write the user's settings.json with the new flag.
fn persist(enabled: bool) -> Result<std::path::PathBuf> {
    let path = settings::user_settings_path();
    let mut raw: RawSettings = if path.exists() {
        let txt = std::fs::read_to_string(&path)?;
        serde_json::from_str(&txt)?
    } else {
        RawSettings::default()
    };
    raw.voice_enabled = Some(enabled);
    settings::write_user_settings(&raw)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bootstrap::SessionId;
    use crate::types::app_state::AppState;

    fn make_ctx() -> CommandContext {
        CommandContext {
            messages: vec![],
            cwd: std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
            app_state: AppState::default(),
            session_id: SessionId::new(),
        }
    }

    #[tokio::test]
    async fn status_shows_disabled_by_default_with_config_path() {
        let handler = VoiceHandler;
        let mut ctx = make_ctx();
        let r = handler.execute("", &mut ctx).await.unwrap();
        match r {
            CommandResult::Output(s) => {
                assert!(s.contains("voiceEnabled:      false"));
                assert!(s.contains("dictation lang:    en"));
                assert!(s.contains("feasibility:       blocked"));
                assert!(s.contains("Config path:"));
            }
            _ => panic!("expected Output"),
        }
    }

    #[tokio::test]
    async fn unknown_subcommand_emits_usage_hint() {
        let handler = VoiceHandler;
        let mut ctx = make_ctx();
        let r = handler.execute("bogus", &mut ctx).await.unwrap();
        match r {
            CommandResult::Output(s) => {
                assert!(s.contains("Unknown /voice subcommand"));
                assert!(s.contains("Usage:"));
            }
            _ => panic!("expected Output"),
        }
    }

    #[tokio::test]
    async fn diagnose_includes_auth_method_and_follow_up_note() {
        let handler = VoiceHandler;
        let mut ctx = make_ctx();
        let r = handler.execute("diagnose", &mut ctx).await.unwrap();
        match r {
            CommandResult::Output(s) => {
                assert!(s.contains("auth method:"));
                assert!(s.contains("language setting:"));
                assert!(s.contains("follow-up"));
            }
            _ => panic!("expected Output"),
        }
    }

    #[tokio::test]
    async fn enable_without_auth_returns_hint() {
        std::env::remove_var("ANTHROPIC_API_KEY");
        std::env::remove_var("ANTHROPIC_AUTH_TOKEN");
        let handler = VoiceHandler;
        let mut ctx = make_ctx();
        let r = handler.execute("on", &mut ctx).await.unwrap();
        match r {
            CommandResult::Output(s) => {
                assert!(s.contains("Voice mode is not available"));
                assert!(s.contains("Hint:"));
            }
            _ => panic!("expected Output"),
        }
    }

    #[test]
    fn short_reason_code_covers_all_feasibility_variants() {
        use crate::voice::audio::AudioUnavailable;
        use crate::voice::stt::SttUnavailable;
        let variants = [
            FeasibilityReason::UnsupportedAuth {
                auth_label: "x".into(),
                message: "y".into(),
            },
            FeasibilityReason::NotAuthenticated("none".into()),
            FeasibilityReason::RemoteEnvironment("remote".into()),
            FeasibilityReason::AudioBackend(AudioUnavailable::NotImplemented("x".into())),
            FeasibilityReason::SttBackend(SttUnavailable::NotImplemented("x".into())),
        ];
        let codes: Vec<_> = variants.iter().map(short_reason_code).collect();
        assert_eq!(codes.len(), 5);
        // All distinct.
        let set: std::collections::HashSet<_> = codes.iter().copied().collect();
        assert_eq!(set.len(), 5);
    }
}
