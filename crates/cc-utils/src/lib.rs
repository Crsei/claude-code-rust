//! Shared utilities extracted from the root crate in Phase 3 (issue #72).
//!
//! These modules are used throughout the codebase but have narrow deps:
//! only `cc-types` (message types) and `cc-config` (bash constants).

pub mod abort;
pub mod bash;
pub mod cwd;
pub mod file_state_cache;
pub mod git;
pub mod messages;
pub mod shell;
pub mod tokens;
