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
//! | `screenshot` | Terminal screenshots: HTML rendering + snapshots  |
//! | `commands`   | Slash commands: /help, /version, /model, etc.    |
//! | `multi_turn` | Multi-turn conversation depth tests              |
//!
//! ## Running
//!
//! ```bash
//! # All tests (require API key in env)
//! cargo test --test pty_ui
//!
//! # Single module
//! cargo test --test pty_ui welcome
//! cargo test --test pty_ui commands
//! cargo test --test pty_ui multi_turn
//!
//! # With output
//! cargo test --test pty_ui -- --nocapture
//! ```
//!
//! ## Log output
//!
//! Each test saves `.raw` (ANSI), `.log` (plain), and `.html` (terminal
//! screenshot) files to `logs/YYYYMMDDHHMM/`.

mod harness;

mod commands;
mod fast_path;
mod input;
mod multi_turn;
mod resize;
mod screenshot;
mod streaming;
mod welcome;
