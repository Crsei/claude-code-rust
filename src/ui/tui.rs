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
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{cursor, execute};
use futures::StreamExt;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::debug;

use crate::engine::lifecycle::QueryEngine;
use crate::engine::sdk_types::SdkMessage;
use crate::types::config::QuerySource;
use crate::types::message::{
    InfoLevel, Message, MessageContent, SystemMessage, SystemSubtype, UserMessage,
};

use super::app::{App, AppAction};

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
    add_welcome_message(&mut app, &engine.session_id, model_name);

    // ── Create channels ────────────────────────────────────────────
    let (engine_tx, mut engine_rx) = mpsc::unbounded_channel::<EngineEvent>();

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
    let mut tick_interval = tokio::time::interval(Duration::from_millis(80));
    tick_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        // Draw the UI
        terminal.draw(|frame| app.render(frame))?;

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
                                // Add user message to display
                                app.add_message(create_user_message(&text));
                                app.push_history(text.clone());
                                app.set_streaming(true);
                                engine.reset_abort();
                                // Start engine query in background
                                spawn_engine_query(
                                    engine.clone(),
                                    text,
                                    engine_tx.clone(),
                                );
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
                            // Scroll actions are handled internally by App
                            _ => {}
                        }
                    }
                    Event::Resize(_, _) => {
                        // ratatui handles resize automatically on next draw
                    }
                    _ => {}
                }
            }

            // Engine events (query results)
            Some(engine_event) = engine_rx.recv() => {
                match engine_event {
                    EngineEvent::Sdk(sdk_msg) => {
                        handle_sdk_message(&mut app, sdk_msg);
                    }
                    EngineEvent::Done => {
                        app.set_streaming(false);
                    }
                }
            }

            // Tick timer (spinner animation, ~80ms)
            _ = tick_interval.tick() => {
                app.tick();
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
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        cursor::Show
    )?;
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
fn handle_sdk_message(app: &mut App, msg: SdkMessage) {
    match msg {
        SdkMessage::SystemInit(_init) => {
            // System init info already shown in the welcome banner.
            debug!("TUI: received SystemInit");
        }

        SdkMessage::Assistant(assistant) => {
            app.add_message(Message::Assistant(assistant.message));
        }

        SdkMessage::Result(result) => {
            app.set_streaming(false);
            app.update_session_cost(result.total_cost_usd);
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

        // UserReplay, StreamEvent, ToolUseSummary: not shown in REPL mode
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Add the startup welcome message to the App.
fn add_welcome_message(app: &mut App, session_id: &str, model_name: &str) {
    let short_session = &session_id[..8.min(session_id.len())];
    let welcome = format!(
        concat!(
            "Claude Code (Rust) v{}\n",
            "Model: {}\n",
            "Session: {}\n",
            "\n",
            "Type your message and press Enter to send.\n",
            "Ctrl+C to abort, Ctrl+D to quit. Up/Down for history.",
        ),
        env!("CARGO_PKG_VERSION"),
        model_name,
        short_session,
    );
    app.add_message(Message::System(SystemMessage {
        uuid: uuid::Uuid::new_v4(),
        timestamp: now_ts(),
        subtype: SystemSubtype::Informational {
            level: InfoLevel::Info,
        },
        content: welcome,
    }));
}

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

/// Current UTC timestamp in seconds.
fn now_ts() -> i64 {
    chrono::Utc::now().timestamp()
}
