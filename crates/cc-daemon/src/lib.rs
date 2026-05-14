//! cc-daemon — KAIROS daemon contracts and runtime owner scaffold.
//!
//! Phase 9 keeps the full daemon process/server implementation in the root
//! binary while this crate starts owning reusable daemon contracts that do not
//! depend on root runtime modules.

pub mod protocol;
