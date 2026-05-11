//! Temporary compatibility shim for the extracted `cc-api` crate.
//!
//! Keep this module until downstream root-crate paths are migrated from
//! `crate::api::*` to `cc_api::api::*`.

pub use cc_api::api::*;
