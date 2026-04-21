//! Background / utility services extracted from the root crate in Phase 3
//! (issue #72).
//!
//! **Partial extraction** — 4 of the 6 services moved cleanly; the two that
//! reach into subsystems still in the root crate stay behind:
//!
//! - `session_analytics` — depends on `session::storage` (`cc-session` is
//!   Phase 4, issue #73).
//! - `langfuse` — depends on `types::tool::Tools`, which can only move once
//!   the tool trait leaves the root crate (Phase 5 hub-cycle break).

pub mod lsp_lifecycle;
pub mod prompt_suggestion;
pub mod session_memory;
pub mod tool_use_summary;
