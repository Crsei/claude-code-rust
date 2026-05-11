//! cc-query - async streaming query loop.
//!
//! This crate owns the query-loop implementation. Root-crate callers import
//! directly from `cc_query`.

pub mod deps;
pub(crate) mod loop_helpers;
pub mod loop_impl;
pub mod stop_hooks;
pub mod token_budget;
pub(crate) mod turn_context;

#[allow(unused_imports)]
pub use cc_engine::agent_runtime::{CompletedBackgroundAgent, PendingBackgroundResults};
#[allow(unused_imports)]
pub use deps::QueryDeps;
#[allow(unused_imports)]
pub use loop_impl::query;
