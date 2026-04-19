//! Integrated TUI runner -- connects the App UI with the QueryEngine.
//!
//! This module bridges the terminal UI (ratatui + crossterm) with the async
//! QueryEngine. It uses:
//!   - A dedicated thread for reading crossterm terminal events
//!   - tokio::spawn tasks for driving engine queries
//!   - mpsc channels for communication between UI and engine
//!
//! The main entry point is [`run_tui`].

use std::io;
use std::sync::Arc;
use std::time::Duration;

use crossterm::event::{self, Event};
use crossterm::terminal::{
    self, BeginSynchronizedUpdate, EndSynchronizedUpdate, EnterAlternateScreen,
    LeaveAlternateScreen,
};
use crossterm::{cursor, execute};
use futures::StreamExt;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::debug;

use crate::commands::{self, CommandContext, CommandResult};
use crate::engine::lifecycle::QueryEngine;
use crate::engine::sdk_types::SdkMessage;
use crate::services::prompt_suggestion::PromptSuggestionService;
use crate::types::config::QuerySource;
use crate::types::message::{
    AssistantMessage, ContentBlock, InfoLevel, Message, MessageContent, StreamEvent, SystemMessage,
    SystemSubtype, UserMessage,
};

use super::app::{App, AppAction};

/// Tracks the partial assistant message being streamed.
struct StreamingState {
    /// Accumulated text content from content_block_delta events.
    text: String,
    /// Whether we are inside a content block.
    active: bool,
}

// ---------------------------------------------------------------------------
// Engine event channel type
// ---------------------------------------------------------------------------

/// Events sent from engine tasks to the TUI main loop.
enum EngineEvent {
    /// An SDK message from the engine stream.
    Sdk(SdkMessage),
    /// The engine query task has completed (stream exhausted).
    Done,
}

// ---------------------------------------------------------------------------
// Terminal guard -- ensures terminal cleanup even on panic
// ---------------------------------------------------------------------------

struct TerminalGuard;

impl TerminalGuard {
    fn new() -> Self {
        Self
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, cursor::Show);
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Run the full TUI application, connecting the App UI with the QueryEngine.
///
/// This function takes ownership of the terminal (raw mode + alternate screen)
/// for its duration, restoring it on exit.
///
/// # Arguments
/// * `engine` - Shared QueryEngine instance
/// * `initial_prompt` - Optional prompt to submit immediately on startup
/// * `model_name` - Model name for display in the status bar
/// * `shutdown_token` - Cancellation token for graceful shutdown
pub async fn run_tui(
    engine: Arc<QueryEngine>,
    initial_prompt: Option<String>,
    model_name: &str,
    shutdown_token: CancellationToken,
) -> anyhow::Result<()> {
    // ── Setup terminal ─────────────────────────────────────────────
    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, cursor::Hide)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Terminal guard ensures cleanup even on early return / panic.
    let _guard = TerminalGuard::new();

    // ── Create the App ─────────────────────────────────────────────
    let mut app = App::new();
    app.set_model_name(model_name.to_string());
    app.set_session_id(engine.session_id.to_string());
    app.set_cwd(engine.cwd().to_string());

    // Scriptable status line (issue #11) — seed from the effective
    // settings snapshot held on AppState. Subsequent edits via
    // `/statusline` reach the App through the command handler.
    //
    // We also adopt the runner handle from AppState so `/statusline`'s
    // status-subcommand reads the live counters from the same instance
    // the TUI is driving (the Arc<Mutex<...>> inside StatusLineRunner
    // makes the clones observe each other).
    {
        let app_state = engine.app_state();
        app.set_status_line_runner(app_state.status_line_runner.clone());
        app.set_status_line_settings(app_state.settings.status_line.clone());
    }

    // Terminal env config (issue #12) — `CLAUDE_CODE_NO_FLICKER`,
    // `CLAUDE_CODE_DISABLE_MOUSE`, `CLAUDE_CODE_SCROLL_SPEED`. Cached
    // for the duration of the session.
    let terminal_env = super::terminal_env::TerminalEnvConfig::from_env();
    app.set_terminal_env(terminal_env);

    // Voice dictation (issue #13) — build a controller from the null
    // backends for now. Real cpal + voice_stream backends can replace
    // these arguments without changing anything above.
    {
        use std::sync::Arc;
        let audio = Arc::new(crate::voice::audio::NullAudioBackend::new());
        let stt = Arc::new(crate::voice::stt::NullTranscriptionClient::new());
        let voice_controller = crate::voice::VoiceController::new(audio, stt);
        app.set_voice_controller(voice_controller);

        let app_state = engine.app_state();
        let lang = crate::voice::language::normalize_language_for_stt(
            app_state.settings.language.as_deref(),
        );
        app.set_voice_settings(app_state.settings.voice_enabled.unwrap_or(false), lang.code);
    }

    // ── Create channels ────────────────────────────────────────────
    let (engine_tx, mut engine_rx) = mpsc::unbounded_channel::<EngineEvent>();
    let mut streaming_state = StreamingState {
        text: String::new(),
        active: false,
    };

    // ── Spawn terminal event reader thread ─────────────────────────
    //
    // crossterm::event::read() is blocking, so we read events in a
    // dedicated OS thread and forward them through an mpsc channel.
    let (term_tx, mut term_rx) = mpsc::unbounded_channel::<Event>();
    std::thread::spawn(move || {
        loop {
            if term_tx.is_closed() {
                break;
            }
            match event::poll(Duration::from_millis(50)) {
                Ok(true) => {
                    if let Ok(evt) = event::read() {
                        if term_tx.send(evt).is_err() {
                            break; // receiver dropped
                        }
                    }
                }
                Ok(false) => {} // timeout, try again
                Err(_) => break,
            }
        }
    });

    // ── Handle initial prompt ──────────────────────────────────────
    if let Some(ref prompt) = initial_prompt {
        app.add_message(create_user_message(prompt));
        app.push_history(prompt.clone());
        app.set_streaming(true);
        engine.reset_abort();
        spawn_engine_query(engine.clone(), prompt.clone(), engine_tx.clone());
    }

    // ── Main event loop ────────────────────────────────────────────
    let mut tick_interval = tokio::time::interval(Duration::from_millis(16));
    tick_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        // Draw the UI only when something changed (dirty flag).
        // `CLAUDE_CODE_NO_FLICKER=0` bypasses synchronized-update escapes
        // (issue #12) for users on terminals that handle them poorly.
        if app.is_dirty() {
            let sync = app.terminal_env().sync_updates;
            if sync {
                execute!(terminal.backend_mut(), BeginSynchronizedUpdate)?;
            }
            terminal.draw(|frame| app.render(frame))?;
            if sync {
                execute!(terminal.backend_mut(), EndSynchronizedUpdate)?;
            }
            app.mark_clean();
        }

        // Wait for the next event
        tokio::select! {
            biased;

            // Shutdown signal (highest priority)
            _ = shutdown_token.cancelled() => {
                debug!("TUI: shutdown signal received");
                break;
            }

            // Terminal events (keys, resize)
            Some(term_event) = term_rx.recv() => {
                match term_event {
                    Event::Key(key) => {
                        let action = app.handle_key_event(key);
                        match action {
                            AppAction::Submit(text) => {
                                app.add_message(create_user_message(&text));
                                app.push_history(text.clone());

                                // Try slash command first
                                if let Some(action) = try_execute_command(
                                    &text, &engine, &mut app
                                ).await {
                                    match action {
                                        CmdAction::Handled => {}
                                        CmdAction::Quit(msg) => {
                                            add_system_info(&mut app, &msg);
                                            break;
                                        }
                                        CmdAction::Query(msgs) => {
                                            // Command wants to send messages to the model
                                            for m in msgs {
                                                app.add_message(m);
                                            }
                                            app.set_streaming(true);
                                            engine.reset_abort();
                                            spawn_engine_query(
                                                engine.clone(),
                                                text,
                                                engine_tx.clone(),
                                            );
                                        }
                                    }
                                } else {
                                    // Regular message — send to engine
                                    app.set_streaming(true);
                                    engine.reset_abort();
                                    spawn_engine_query(
                                        engine.clone(),
                                        text,
                                        engine_tx.clone(),
                                    );
                                }
                            }
                            AppAction::Abort => {
                                engine.abort();
                                app.set_streaming(false);
                                app.add_message(Message::System(SystemMessage {
                                    uuid: uuid::Uuid::new_v4(),
                                    timestamp: now_ts(),
                                    subtype: SystemSubtype::Informational {
                                        level: InfoLevel::Warning,
                                    },
                                    content: "Aborted by user".to_string(),
                                }));
                            }
                            AppAction::Quit => {
                                debug!("TUI: quit requested");
                                break;
                            }
                            AppAction::ExportTranscript(body) => {
                                match export_to_editor(&body).await {
                                    Ok(path) => add_system_info(
                                        &mut app,
                                        &format!(
                                            "Transcript exported to {}",
                                            path.display()
                                        ),
                                    ),
                                    Err(e) => add_system_info(
                                        &mut app,
                                        &format!("Transcript export failed: {}", e),
                                    ),
                                }
                            }
                            // Scroll actions are handled internally by App
                            _ => {}
                        }
                    }
                    Event::Resize(_, _) => {
                        app.mark_dirty();
                    }
                    _ => {}
                }
            }

            // Engine events (query results)
            Some(engine_event) = engine_rx.recv() => {
                match engine_event {
                    EngineEvent::Sdk(sdk_msg) => {
                        handle_sdk_message(&mut app, sdk_msg, &mut streaming_state);
                    }
                    EngineEvent::Done => {
                        app.set_streaming(false);
                    }
                }
            }

            // Tick timer (spinner animation, ~80ms)
            _ = tick_interval.tick() => {
                app.tick();
                // Drain any pending voice-controller events (issue #13).
                // Returns true when something was inserted into the
                // prompt or the state changed — both cases need a redraw.
                app.drain_voice_events();
            }
        }

        if app.should_quit() {
            break;
        }
    }

    // ── Restore terminal ───────────────────────────────────────────
    // (TerminalGuard::drop also handles this, but explicit cleanup is
    // cleaner for the normal exit path.)
    terminal::disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, cursor::Show)?;
    terminal.show_cursor()?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Engine query task
// ---------------------------------------------------------------------------

/// Spawn a tokio task that drives a QueryEngine query and sends results
/// through the `tx` channel.
fn spawn_engine_query(
    engine: Arc<QueryEngine>,
    prompt: String,
    tx: mpsc::UnboundedSender<EngineEvent>,
) {
    tokio::spawn(async move {
        let stream = engine.submit_message(&prompt, QuerySource::ReplMainThread);
        futures::pin_mut!(stream);

        while let Some(msg) = stream.next().await {
            if tx.send(EngineEvent::Sdk(msg)).is_err() {
                break; // receiver dropped (app exited)
            }
        }

        let _ = tx.send(EngineEvent::Done);
    });
}

// ---------------------------------------------------------------------------
// SDK message handler
// ---------------------------------------------------------------------------

/// Handle an SDK message from the engine, updating the App state.
fn handle_sdk_message(app: &mut App, msg: SdkMessage, ss: &mut StreamingState) {
    match msg {
        SdkMessage::SystemInit(_init) => {
            debug!("TUI: received SystemInit");
        }

        SdkMessage::StreamEvent(sdk_stream) => {
            match sdk_stream.event {
                StreamEvent::ContentBlockStart { .. } => {
                    if !ss.active {
                        // First content block — start a new streaming message
                        ss.text.clear();
                        ss.active = true;
                        app.add_message(make_partial_assistant(""));
                    }
                }
                StreamEvent::ContentBlockDelta { ref delta, .. } => {
                    if let Some(t) = delta.get("text").and_then(|v| v.as_str()) {
                        ss.text.push_str(t);
                        app.replace_last_message(make_partial_assistant(&ss.text));
                    }
                }
                StreamEvent::MessageStop => {
                    // Stream complete; the full Assistant message follows.
                    ss.active = false;
                }
                _ => {}
            }
        }

        SdkMessage::Assistant(assistant) => {
            // Replace the partial streaming message with the final one.
            if ss.active || !ss.text.is_empty() {
                app.replace_last_message(Message::Assistant(assistant.message));
                ss.text.clear();
                ss.active = false;
            } else {
                app.add_message(Message::Assistant(assistant.message));
            }
        }

        SdkMessage::UserReplay(user) => {
            if user.is_replay && !user.is_synthetic {
                return;
            }

            let content = match user.content_blocks {
                Some(blocks) => MessageContent::Blocks(blocks),
                None => MessageContent::Text(user.content),
            };
            app.add_message(Message::User(UserMessage {
                uuid: user.uuid,
                timestamp: user.timestamp,
                role: "user".to_string(),
                content,
                is_meta: user.is_synthetic,
                tool_use_result: None,
                source_tool_assistant_uuid: None,
            }));
        }

        SdkMessage::Result(result) => {
            // Finalize any leftover streaming state
            ss.text.clear();
            ss.active = false;

            app.set_streaming(false);
            app.update_session_cost(result.total_cost_usd);
            // Feed aggregate usage into the status-line payload (issue #11).
            // `result.usage` is engine `UsageTracking` (accumulated across
            // turns) — the payload wants per-session totals, so we pass
            // the totals straight through.
            app.update_session_usage(
                result.usage.total_input_tokens,
                result.usage.total_output_tokens,
                result.usage.total_cache_read_tokens,
                result.usage.total_cache_creation_tokens,
                result.usage.api_call_count,
            );
            if result.is_error {
                app.add_message(Message::System(SystemMessage {
                    uuid: uuid::Uuid::new_v4(),
                    timestamp: now_ts(),
                    subtype: SystemSubtype::Informational {
                        level: InfoLevel::Error,
                    },
                    content: result.result,
                }));
            }

            // Generate next-prompt suggestions from last assistant turn
            generate_suggestions(app);

            debug!(
                turns = result.num_turns,
                cost = format!("{:.4}", result.total_cost_usd),
                duration_ms = result.duration_ms,
                "TUI: query completed"
            );
        }

        SdkMessage::ApiRetry(retry) => {
            app.set_spinner_message(format!(
                "Retrying ({}/{})...",
                retry.attempt, retry.max_retries
            ));
        }

        SdkMessage::CompactBoundary(_) => {
            app.add_message(Message::System(SystemMessage {
                uuid: uuid::Uuid::new_v4(),
                timestamp: now_ts(),
                subtype: SystemSubtype::CompactBoundary {
                    compact_metadata: None,
                },
                content: String::new(),
            }));
        }

        _ => {}
    }
}

/// Build a partial assistant message for streaming display.
fn make_partial_assistant(text: &str) -> Message {
    Message::Assistant(AssistantMessage {
        uuid: uuid::Uuid::new_v4(),
        timestamp: now_ts(),
        role: "assistant".to_string(),
        content: vec![ContentBlock::Text {
            text: text.to_string(),
        }],
        usage: None,
        stop_reason: None,
        is_api_error_message: false,
        api_error: None,
        cost_usd: 0.0,
    })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Create a user message from text.
fn create_user_message(text: &str) -> Message {
    Message::User(UserMessage {
        uuid: uuid::Uuid::new_v4(),
        timestamp: now_ts(),
        role: "user".to_string(),
        content: MessageContent::Text(text.to_string()),
        is_meta: false,
        tool_use_result: None,
        source_tool_assistant_uuid: None,
    })
}

/// Generate prompt suggestions from the last assistant message in the conversation.
fn generate_suggestions(app: &mut super::app::App) {
    let messages = app.messages();
    let mut svc = PromptSuggestionService::new(true);

    // Check suppression first (not enough messages, etc.)
    if let Some(reason) = svc.get_suppression_reason(messages.len(), false) {
        debug!("prompt suggestions suppressed: {:?}", reason);
        return;
    }

    if !svc.should_enable() {
        return;
    }

    // Find last assistant message
    let last_assistant = messages.iter().rev().find_map(|msg| match msg {
        Message::Assistant(a) => Some(a),
        _ => None,
    });
    let Some(assistant) = last_assistant else {
        return;
    };

    // Extract tool names and text summary
    let tool_names: Vec<String> = assistant
        .content
        .iter()
        .filter_map(|b| {
            if let ContentBlock::ToolUse { name, .. } = b {
                Some(name.clone())
            } else {
                None
            }
        })
        .collect();

    let summary: String = assistant
        .content
        .iter()
        .filter_map(|b| {
            if let ContentBlock::Text { text } = b {
                Some(text.as_str())
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    if let Some(suggestions) = svc.try_generate(&summary, &tool_names) {
        app.set_suggestions(suggestions);
    }
}

/// Current UTC timestamp in seconds.
fn now_ts() -> i64 {
    chrono::Utc::now().timestamp()
}

// ---------------------------------------------------------------------------
// Slash-command execution
// ---------------------------------------------------------------------------

/// Internal action returned after executing a slash command.
enum CmdAction {
    /// Command fully handled, output already added to app.
    Handled,
    /// Command requested exit.
    Quit(String),
    /// Command produced messages to send to the model.
    Query(Vec<Message>),
}

fn conversation_changed(before: &[Message], after: &[Message]) -> bool {
    before.len() != after.len()
        || before
            .iter()
            .zip(after.iter())
            .any(|(lhs, rhs)| lhs.uuid() != rhs.uuid())
}

fn replace_app_messages(app: &mut App, messages: &[Message]) {
    app.clear_messages();
    for message in messages {
        app.add_message(message.clone());
    }
}

/// Try to execute a slash command. Returns `None` if the input is not a command.
async fn try_execute_command(
    text: &str,
    engine: &Arc<QueryEngine>,
    app: &mut App,
) -> Option<CmdAction> {
    let trimmed = text.trim();
    if !trimmed.starts_with('/') {
        return None;
    }

    let (cmd_idx, args) = commands::parse_command_input(trimmed)?;
    let all_commands = commands::get_all_commands();
    let cmd = &all_commands[cmd_idx];
    let original_messages = engine.messages();

    let mut ctx = CommandContext {
        messages: original_messages.clone(),
        cwd: std::path::PathBuf::from(engine.cwd()),
        app_state: engine.app_state(),
        session_id: engine.session_id.clone(),
    };

    match cmd.handler.execute(&args, &mut ctx).await {
        Ok(result) => match result {
            CommandResult::Output(text) => {
                if conversation_changed(&original_messages, &ctx.messages) {
                    engine.replace_messages(ctx.messages.clone());
                    replace_app_messages(app, &ctx.messages);
                }
                // Propagate settings mutations (e.g. `/voice on/off`,
                // `/statusline set`) back to the engine's AppState and
                // refresh App's cached voice snapshot so push-to-talk
                // picks up the change without restart.
                let voice_enabled = ctx.app_state.settings.voice_enabled.unwrap_or(false);
                let lang = crate::voice::language::normalize_language_for_stt(
                    ctx.app_state.settings.language.as_deref(),
                );
                let lang_code = lang.code.clone();
                engine.update_app_state(|s| {
                    s.settings.voice_enabled = Some(voice_enabled);
                    s.settings.language = ctx.app_state.settings.language.clone();
                });
                app.set_voice_settings(voice_enabled, lang_code);
                add_system_info(app, &text);
                Some(CmdAction::Handled)
            }
            CommandResult::Clear => {
                // Clear conversation in the engine and the app
                engine.clear_messages();
                app.clear_messages();
                add_system_info(app, "Conversation cleared.");
                Some(CmdAction::Handled)
            }
            CommandResult::Exit(msg) => Some(CmdAction::Quit(msg)),
            CommandResult::Query(msgs) => {
                if conversation_changed(&original_messages, &ctx.messages) {
                    engine.replace_messages(ctx.messages.clone());
                    replace_app_messages(app, &ctx.messages);
                }
                Some(CmdAction::Query(msgs))
            }
            CommandResult::None => {
                if conversation_changed(&original_messages, &ctx.messages) {
                    engine.replace_messages(ctx.messages.clone());
                    replace_app_messages(app, &ctx.messages);
                }
                Some(CmdAction::Handled)
            }
        },
        Err(e) => {
            add_system_error(app, &format!("Command error: {e}"));
            Some(CmdAction::Handled)
        }
    }
}

/// Add an informational system message to the app.
fn add_system_info(app: &mut App, text: &str) {
    app.add_message(Message::System(SystemMessage {
        uuid: uuid::Uuid::new_v4(),
        timestamp: now_ts(),
        subtype: SystemSubtype::Informational {
            level: InfoLevel::Info,
        },
        content: text.to_string(),
    }));
}

/// Export a pre-rendered transcript body to a temp file and open it in
/// `$VISUAL` / `$EDITOR`. Returns the path on success. The TUI exits
/// alternate screen while the editor runs so the user can scroll freely,
/// and re-enters before returning to the main loop.
///
/// Falls back to "just write the file" when no editor env var is set.
async fn export_to_editor(body: &str) -> anyhow::Result<std::path::PathBuf> {
    use std::io::Write as _;
    let mut path = std::env::temp_dir();
    let stem = format!(
        "cc-rust-transcript-{}.md",
        chrono::Utc::now().format("%Y%m%d-%H%M%S")
    );
    path.push(stem);
    {
        let mut f = std::fs::File::create(&path)?;
        f.write_all(body.as_bytes())?;
    }

    let editor = std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .ok();
    if let Some(ed) = editor.filter(|s| !s.trim().is_empty()) {
        // Leave the alternate screen so the editor can paint over a real
        // terminal. Re-entering on the way out is handled by the caller
        // via the dirty flag + next render.
        let _ = execute!(std::io::stdout(), LeaveAlternateScreen, cursor::Show);
        let _ = terminal::disable_raw_mode();

        let status = tokio::process::Command::new(&ed).arg(&path).status().await;

        // Always re-arm the terminal, even on editor failure.
        let _ = terminal::enable_raw_mode();
        let _ = execute!(std::io::stdout(), EnterAlternateScreen, cursor::Hide);

        match status {
            Ok(s) if s.success() => {}
            Ok(s) => {
                return Err(anyhow::anyhow!(
                    "{} exited with status {}",
                    ed,
                    s.code().unwrap_or(-1)
                ))
            }
            Err(e) => return Err(anyhow::anyhow!("could not launch '{}': {}", ed, e)),
        }
    }
    Ok(path)
}

/// Add an error system message to the app.
fn add_system_error(app: &mut App, text: &str) {
    app.add_message(Message::System(SystemMessage {
        uuid: uuid::Uuid::new_v4(),
        timestamp: now_ts(),
        subtype: SystemSubtype::Informational {
            level: InfoLevel::Error,
        },
        content: text.to_string(),
    }));
}
