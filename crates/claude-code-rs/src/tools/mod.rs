//! Tool registry entry point.
//!
//! Concrete tool implementations now live with their root-domain owners while
//! they wait for extraction into `cc-*` crates. This module keeps the runtime
//! registry wiring in one place.

pub mod registry;
