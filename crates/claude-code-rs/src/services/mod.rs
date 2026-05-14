//! Services module - background and utility services for cc-rust.
//!
//! Shared services live in `cc-services`; root keeps only services still tied
//! to root command and daemon wiring.

pub mod onboarding;
pub mod scheduler;
pub mod session_analytics;
