//! Placeholder: UI capability contract.
//!
//! Purpose:
//! - Define the canonical mapping between backend events, frontend commands,
//!   and visible terminal UI surfaces.
//! - Use this before wiring new UI panels so `ui/src` and the Rust TUI do not
//!   drift into incompatible event semantics.
//!
//! Reference paths:
//! - docs/ui-parity-update-plan.md
//! - ui/src/ipc/protocol.ts
//! - ui/src/store/app-state.ts
//! - ui/src/store/app-store.tsx
//! - crates/claude-code-rs/src/ui/tui.rs
//! - crates/claude-code-rs/src/ui/app.rs
//!
//! Implementation note:
//! - Keep this file as comments only until the contract is expressed as Rust
//!   types and covered by IPC contract tests.

