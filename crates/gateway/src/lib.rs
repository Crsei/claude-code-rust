//! Remote-control gateway foundation.
//!
//! This crate owns remote source identity, deterministic session-key
//! derivation, and gateway-local configuration models. Runtime daemon,
//! TUI, IPC, and `QueryEngine` integration belong in `claude-code-rs`
//! adapter modules so dependency direction stays one-way.

pub mod config;
pub mod session_key;
pub mod source;

pub use config::{GatewayConfig, GatewayLimits, GatewayPersistence};
pub use session_key::{SessionKey, SessionKeyPolicy};
pub use source::{RemoteSource, RemoteSourceMetadata, RemoteTransport};
