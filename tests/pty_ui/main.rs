//! PTY-based UI integration tests.
//!
//! Tests the ratatui TUI by spawning `claude-code-rs` in a real
//! pseudo-terminal (ConPTY on Windows) and capturing / asserting on
//! the rendered terminal output.
//!
//! ## Module layout
//!
//! | Module       | What it tests                                    |
//! |--------------|--------------------------------------------------|
//! | `harness`    | Shared `PtySession` helper (not tests)           |
//! | `fast_path`  | `--version`, `--init-only`, `--dump-system-prompt`, `-p` |
//! | `welcome`    | Welcome screen: logo, model, session, tips       |
//! | `input`      | Input prompt: typing, cursor, Ctrl keys, vim     |
//! | `streaming`  | Streaming lifecycle, abort, multi-turn, tool use |
//! | `resize`     | Terminal resize behavior                         |
//!
//! ## Running
//!
//! ```bash
//! # All tests (require API key in env)
//! cargo test --test pty_ui
//!
//! # Single module
//! cargo test --test pty_ui welcome
//!
//! # With output
//! cargo test --test pty_ui -- --nocapture
//! ```
//!
//! ## Log output
//!
//! Each test saves `.raw` (ANSI) and `.log` (plain) files to
//! `logs/YYYYMMDDHHMM/`.

mod harness;

mod fast_path;
mod welcome;
mod input;
mod streaming;
mod resize;
