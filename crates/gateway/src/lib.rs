//! Remote-control gateway foundation.
//!
//! This crate owns remote source identity, deterministic session-key
//! derivation, and gateway-local configuration models. Runtime daemon,
//! TUI, IPC, and `QueryEngine` integration belong in `claude-code-rs`
//! adapter modules so dependency direction stays one-way.

pub mod config;
pub mod events;
pub mod policy;
pub mod run;
pub mod session_key;
pub mod source;
pub mod store;

pub use config::{GatewayConfig, GatewayLimits, GatewayPersistence};
pub use events::{RunEvent, RunEventKind};
pub use policy::{BusyDecision, BusySnapshot, GatewayPolicy};
pub use run::{
    BusyPolicy, CreateRunOutcome, GatewayDiagnostic, GatewayError, RunId, RunMeta, RunPolicy,
    RunRequest, RunStatus,
};
pub use session_key::{SessionKey, SessionKeyPolicy};
pub use source::{RemoteSource, RemoteSourceMetadata, RemoteTransport};
pub use store::GatewayStore;
