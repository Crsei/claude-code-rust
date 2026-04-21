// `message`, `state`, `transitions` now live in the `cc-types` workspace crate
// (issue #70 — Phase 1 leaf extraction). Re-export them here so existing
// `crate::types::message::*` paths keep resolving across the ~100 call sites
// in this crate.
//
// `app_state`, `tool`, and `config` still depend on teams / ui / config / ipc
// and stay local to the root crate until those subsystems move out. Once they
// do, this file can collapse to a single `pub use cc_types::*;`.
pub use cc_types::{message, state, transitions};

pub mod app_state;
pub mod config;
pub mod tool;
