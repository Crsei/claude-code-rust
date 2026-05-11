//! cc-query - async streaming query loop.
//!
//! This crate owns the query-loop implementation. The root binary keeps a
//! temporary `crate::query` shim so older call sites can continue to compile
//! while the workspace split proceeds.

pub mod deps;
pub(crate) mod loop_helpers;
pub mod loop_impl;
pub mod stop_hooks;
pub mod token_budget;
pub(crate) mod turn_context;

#[allow(unused_imports)]
pub use cc_types::background_agents::{CompletedBackgroundAgent, PendingBackgroundResults};
#[allow(unused_imports)]
pub use deps::QueryDeps;
#[allow(unused_imports)]
pub use loop_impl::query;
