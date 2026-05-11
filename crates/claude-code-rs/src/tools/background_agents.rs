//! Background agent runtime types re-exported from `cc-engine`.
//!
//! The real definitions live with the engine lifecycle because they bridge
//! background Agent runtime events into query-turn injection. This module is
//! kept only as a compatibility shim for legacy root-crate imports.

#[allow(unused_imports)]
pub use cc_engine::agent_runtime::{CompletedBackgroundAgent, PendingBackgroundResults};
