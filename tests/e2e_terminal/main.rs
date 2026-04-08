//! E2E tests for the terminal launch flow (--headless IPC protocol).
//!
//! These tests simulate what `cc-rust` / `run.ps1` / `main.tsx` does:
//! spawn the Rust binary with `--headless` and communicate via JSONL
//! over stdin/stdout.
//!
//! Structure:
//!   helpers.rs      — spawn_headless, read_line_json, send_msg, collect_until
//!   offline.rs      — 12 offline IPC protocol tests (no API key needed)
//!   live.rs         — 4 live streaming lifecycle tests (#[ignore])
//!   permission.rs   — permission mode tests (1 offline + 3 live)
//!   tool_display.rs — tool_use visibility + large file truncation (5 live)
//!   usage.rs        — token/cost tracking validation (4 live)
//!   commands.rs     — slash command handling (13 offline)
//!
//! Run offline:  cargo test --test e2e_terminal
//! Run live:     cargo test --test e2e_terminal -- --ignored
//! Run module:   cargo test --test e2e_terminal commands
//! Run single:   cargo test --test e2e_terminal usage::usage_update_has_nonzero_tokens -- --ignored

mod helpers;
mod commands;
mod live;
mod offline;
mod permission;
mod tool_display;
mod usage;
