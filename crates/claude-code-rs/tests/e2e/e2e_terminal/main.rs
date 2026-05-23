//! E2E tests for the terminal launch flow (--headless IPC protocol).
//!
//! These tests simulate what `cc-rust` / `run.ps1` / `main.tsx` does:
//! spawn the Rust binary with `--headless` and communicate via JSONL
//! over stdin/stdout.
//!
//! Structure:
//!   helpers.rs      鈥?spawn_headless, read_line_json, send_msg, collect_until
//!   offline.rs      鈥?12 offline IPC protocol tests (no API key needed)
//!   live.rs         鈥?4 live streaming lifecycle tests (#[ignore])
//!   permission.rs   鈥?permission mode tests (1 offline + 3 live)
//!   tool_display.rs 鈥?tool_use visibility + large file truncation (5 live)
//!   usage.rs        鈥?token/cost tracking validation (4 live)
//!   commands.rs     鈥?slash command handling (13 offline)
//!   phase6.rs      - coordinator/team/tasks + worktree hook regressions
//!
//! Run offline:  cargo test --test e2e_terminal
//! Run live:     cargo test --test e2e_terminal -- --ignored
//! Run module:   cargo test --test e2e_terminal commands
//! Run single:   cargo test --test e2e_terminal usage::usage_update_has_nonzero_tokens -- --ignored

mod commands;
mod helpers;
mod live;
mod offline;
mod permission;
mod phase6;
mod subagent;
mod tool_display;
mod usage;
