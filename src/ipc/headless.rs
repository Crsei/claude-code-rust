//! Headless event loop — replaces `tui::run_tui()` when `--headless` is passed.
//!
//! Communicates with an external UI process via JSON lines on stdin/stdout
//! using the protocol defined in [`super::protocol`].
//!
//! This module is now a thin entry point.  The real work is done by:
//! - [`super::runtime::HeadlessRuntime`] — owns the `select!` loop and state
//! - [`super::callbacks`] — installs permission/question callbacks
//! - [`super::ingress`] — dispatches [`FrontendMessage`]s

use std::sync::Arc;

use crate::engine::lifecycle::QueryEngine;

use super::runtime::HeadlessRuntime;
use super::sink::FrontendSink;

/// Run the headless event loop.
///
/// Reads [`FrontendMessage`]s from stdin (one JSON object per line) and writes
/// [`BackendMessage`]s to stdout.  This function only returns when the UI sends
/// `Quit` or stdin is closed.
pub async fn run_headless(engine: Arc<QueryEngine>, model: String) -> anyhow::Result<()> {
    let sink = FrontendSink::stdout();
    let runtime = HeadlessRuntime::new(engine, sink);
    runtime.run(model).await
}
