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

use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event};
use crossterm::terminal::{
    self, BeginSynchronizedUpdate, EndSynchronizedUpdate, EnterAlternateScreen,
    LeaveAlternateScreen,
};
use crossterm::{cursor, execute};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;
use tracing::debug;

#[path = "tui/commands.rs"]
mod commands;
#[path = "tui/engine_events.rs"]
mod engine_events;
#[path = "tui/export.rs"]
mod export;
#[path = "tui/subsystem_events.rs"]
mod subsystem_events;
#[path = "tui/terminal_guard.rs"]
mod terminal_guard;
#[cfg(test)]
#[path = "tui/tests.rs"]
mod tests;

use commands::{query_prompt_text, try_execute_command, CmdAction};
use engine_events::{
    create_user_message, handle_sdk_message, install_tui_permission_callback, now_ts,
    permission_choice_to_decision, spawn_engine_query, EngineEvent, StreamingState,
};
use export::export_to_editor;
use subsystem_events::{
    add_system_error, add_system_info, handle_lsp_recommendation_response, handle_subsystem_event,
};
use terminal_guard::TerminalGuard;

use crate::engine::lifecycle::QueryEngine;
use crate::ipc::subsystem_events::SubsystemEventBus;
use crate::types::message::{InfoLevel, Message, SystemMessage, SystemSubtype};

use super::app::{App, AppAction};

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
    let terminal_env = super::terminal_env::TerminalEnvConfig::from_env();
    let mouse_capture_enabled = !terminal_env.disable_mouse;

    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    if mouse_capture_enabled {
        execute!(
            stdout,
            EnterAlternateScreen,
            EnableMouseCapture,
            cursor::Hide
        )?;
    } else {
        execute!(stdout, EnterAlternateScreen, cursor::Hide)?;
    }
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Terminal guard ensures cleanup even on early return / panic.
    let _guard = TerminalGuard::new(mouse_capture_enabled);

    // ── Create the App ─────────────────────────────────────────────
    let mut app = App::new();
    app.set_model_name(model_name.to_string());
    app.set_backend_name(engine.app_state().main_loop_backend.clone());
    app.set_session_id(engine.current_session_id().to_string());
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
        app.set_keybindings(app_state.keybindings.clone());
        app.set_editor_mode(app_state.settings.editor_mode.as_deref());
        app.set_output_style(app_state.settings.output_style.clone());
    }

    // Terminal env config (issue #12) — `CLAUDE_CODE_NO_FLICKER`,
    // `CLAUDE_CODE_DISABLE_MOUSE`, `CLAUDE_CODE_SCROLL_SPEED`. Cached
    // for the duration of the session.
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
        let voice_supported = matches!(
            crate::commands::voice_cmd::current_feasibility(),
            crate::voice::feasibility::Feasibility::Ready { .. }
        );
        app.set_voice_settings(
            app_state.settings.voice_enabled.unwrap_or(false),
            lang.code,
            voice_supported,
        );
    }

    // ── Create channels ────────────────────────────────────────────
    let (engine_tx, mut engine_rx) = mpsc::unbounded_channel::<EngineEvent>();
    install_tui_permission_callback(&engine, engine_tx.clone());
    let mut pending_permission_response: Option<oneshot::Sender<String>> = None;
    let mut streaming_state = StreamingState::new();

    let subsystem_bus = SubsystemEventBus::new();
    let mut subsystem_rx = subsystem_bus.subscribe();
    crate::lsp_service::set_event_sender(subsystem_bus.sender());

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
                                            let prompt = query_prompt_text(&msgs);
                                            for m in msgs {
                                                app.add_message(m);
                                            }
                                            app.set_streaming(true);
                                            engine.reset_abort();
                                            spawn_engine_query(
                                                engine.clone(),
                                                if prompt.trim().is_empty() {
                                                    text
                                                } else {
                                                    prompt
                                                },
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
                            AppAction::PermissionResponse(choice) => {
                                if let Some(response_tx) = pending_permission_response.take() {
                                    let _ = response_tx
                                        .send(permission_choice_to_decision(choice).to_string());
                                }
                            }
                            AppAction::LspRecommendationResponse {
                                request_id,
                                plugin_name,
                                decision,
                            } => {
                                handle_lsp_recommendation_response(
                                    &mut app,
                                    request_id,
                                    plugin_name,
                                    decision,
                                );
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
                    Event::Mouse(mouse) => {
                        let _ = app.handle_mouse_event(mouse);
                    }
                    _ => {}
                }
            }

            // Engine events (query results)
            Some(engine_event) = engine_rx.recv() => {
                match engine_event {
                    EngineEvent::Sdk(sdk_msg) => {
                        handle_sdk_message(&mut app, *sdk_msg, &mut streaming_state);
                    }
                    EngineEvent::PermissionRequest {
                        tool_name,
                        description,
                        response_tx,
                    } => {
                        if let Some(previous) = pending_permission_response.take() {
                            let _ = previous.send("deny".to_string());
                        }
                        app.show_permission_dialog(&tool_name, "", &description);
                        pending_permission_response = Some(response_tx);
                    }
                    EngineEvent::Done => {
                        app.set_streaming(false);
                    }
                }
            }

            subsystem_event = subsystem_rx.recv() => {
                match subsystem_event {
                    Ok(event) => handle_subsystem_event(&mut app, event),
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        add_system_error(&mut app, "A subsystem UI event was dropped because the TUI fell behind.");
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {}
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
    if mouse_capture_enabled {
        execute!(
            terminal.backend_mut(),
            DisableMouseCapture,
            LeaveAlternateScreen,
            cursor::Show
        )?;
    } else {
        execute!(terminal.backend_mut(), LeaveAlternateScreen, cursor::Show)?;
    }
    terminal.show_cursor()?;

    Ok(())
}
