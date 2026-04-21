//! Services module — background and utility services for cc-rust.
//!
//! Most services have been moved into the `cc-services` workspace crate
//! (Phase 3, issue #72). The two exceptions reach into subsystems still in
//! the root crate and stay here until those move:
//!
//! - [`session_analytics`] — depends on `session::storage`
//!   (unblocked by Phase 4, issue #73).
//! - [`langfuse`] — depends on `types::tool::Tools`
//!   (unblocked by Phase 5 hub-cycle break).
//!
//! The `pub use cc_services::*;` re-export keeps every historical
//! `crate::services::...` path working.

pub use cc_services::*;

pub mod langfuse;
pub mod session_analytics;
