//! Compatibility shim for the QueryEngine lifecycle source now housed in
//! `cc-engine`.

#[path = "../../../../cc-engine/src/lifecycle/mod.rs"]
mod extracted;

pub use extracted::*;
