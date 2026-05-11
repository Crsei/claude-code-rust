//! Services module - background and utility services for cc-rust.
//!
//! Most services have been moved into the `cc-services` workspace crate. The
//! root module re-exports them so historical `crate::services::...` paths keep
//! working while remaining root-local services are extracted.

pub use cc_services::*;

pub mod onboarding;
pub mod scheduler;
pub mod session_analytics;
