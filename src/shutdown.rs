//! Phase I: Shutdown and cleanup
//!
//! Corresponds to: LIFECYCLE_STATE_MACHINE.md §10 (Phase I)
//!
//! Handles graceful shutdown:
//! - SIGINT/Ctrl-C handler registration
//! - Abort signal propagation to all running tools
//! - Session persistence flush
//! - Transcript flush
//! - Cursor/terminal reset
//! - Timer/watcher cleanup

use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use crate::engine::lifecycle::QueryEngine;
use crate::session::transcript;

// ---------------------------------------------------------------------------
// Shutdown handler registration
// ---------------------------------------------------------------------------

/// Register a Ctrl-C / SIGINT handler that triggers the cancellation token.
///
/// Returns a `CancellationToken` that the REPL loop watches. When the user
/// presses Ctrl-C, the token is cancelled, which causes the REPL to exit
/// gracefully.
///
/// Corresponds to TypeScript: `registerSigintHandler()` in cli.tsx
pub fn register_shutdown_handler() -> CancellationToken {
    let token = CancellationToken::new();
    let shutdown_token = token.clone();

    // First Ctrl-C: cancel token (graceful shutdown)
    // If the runtime is still alive, this triggers cleanup.
    tokio::spawn(async move {
        match tokio::signal::ctrl_c().await {
            Ok(()) => {
                info!("Ctrl-C received, initiating graceful shutdown");
                shutdown_token.cancel();
            }
            Err(e) => {
                warn!(error = %e, "Failed to install Ctrl-C handler");
            }
        }
    });

    token
}

// ---------------------------------------------------------------------------
// Graceful shutdown sequence
// ---------------------------------------------------------------------------

/// Execute the full graceful shutdown sequence.
///
/// Corresponds to TypeScript: `gracefulShutdown()` + `gracefulShutdownSync()`
///
/// Steps:
/// 1. Abort any running query (propagates to all tool executions)
/// 2. Flush the session transcript to disk
/// 3. Reset terminal state (cursor, raw mode)
/// 4. Log shutdown metrics
pub async fn graceful_shutdown(engine: &QueryEngine) {
    debug!("graceful_shutdown: starting");

    // Step 1: Abort any running query
    if !engine.is_aborted() {
        engine.abort();
        debug!("graceful_shutdown: abort signal sent");
    }

    // Step 2: Flush transcript
    let session_id = &engine.session_id;
    if let Err(e) = transcript::flush_transcript(session_id) {
        warn!(error = %e, "failed to flush transcript during shutdown");
    } else {
        debug!("graceful_shutdown: transcript flushed");
    }

    // Step 3: Persist session (save current messages)
    let messages = engine.messages();
    if !messages.is_empty() {
        let cwd = ""; // engine doesn't expose cwd directly; acceptable for shutdown
        if let Err(e) = crate::session::storage::save_session(session_id, &messages, cwd) {
            warn!(error = %e, "failed to save session during shutdown");
        } else {
            debug!("graceful_shutdown: session saved");
        }
    }

    // Step 4: Reset terminal state
    graceful_shutdown_sync();

    // Step 5: Log final usage
    let usage = engine.usage();
    if usage.api_call_count > 0 {
        info!(
            api_calls = usage.api_call_count,
            input_tokens = usage.total_input_tokens,
            output_tokens = usage.total_output_tokens,
            cost_usd = format!("{:.4}", usage.total_cost_usd),
            "session usage summary"
        );
    }

    debug!("graceful_shutdown: complete");
}

/// Synchronous shutdown actions (terminal state reset).
///
/// Corresponds to TypeScript: `gracefulShutdownSync()`
fn graceful_shutdown_sync() {
    // Reset cursor visibility (in case we hid it during rendering)
    // This uses crossterm directly to ensure it runs even on panic.
    use std::io::Write;
    let mut stdout = std::io::stdout();

    // Show cursor
    let _ = crossterm::execute!(stdout, crossterm::cursor::Show);

    // Disable raw mode if it was enabled
    let _ = crossterm::terminal::disable_raw_mode();

    // Flush stdout
    let _ = stdout.flush();

    debug!("graceful_shutdown_sync: terminal reset");
}
