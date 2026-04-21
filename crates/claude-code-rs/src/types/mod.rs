// `app_state`, `tool`, and `config` moved to `cc-engine::types` in Phase 6
// (issue #75). `message`, `state`, `transitions` live in `cc-types` (Phase 1).
// This module re-exports both sets so existing
// `crate::types::{app_state, tool, config, message, state, transitions}`
// paths across the root crate keep compiling unchanged.

pub use cc_engine::types::{app_state, config, tool};
pub use cc_types::{message, state, transitions};
