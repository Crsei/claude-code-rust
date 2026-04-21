//! Background agent types — re-exported from `cc-types::background_agents`.
//!
//! The real definitions moved to cc-types in Phase 6 to break the
//! `query -> tools` edge. This module is kept as a thin re-export so existing
//! call sites (engine, ipc, agent tool) continue to work unchanged.

#[allow(unused_imports)]
pub use cc_types::background_agents::{CompletedBackgroundAgent, PendingBackgroundResults};
