//! Langfuse observability via OpenTelemetry.
//!
//! This module keeps Langfuse-specific wiring out of the business logic and
//! exposes a small set of helpers for lifecycle boundaries (submit/model/tool).

pub mod client;
pub mod convert;
pub mod sanitize;
pub mod tracing;

pub use client::{init_langfuse, shutdown_langfuse};
pub use tracing::{
    create_generation_span, create_subagent_trace, create_tool_batch_span, create_tool_span,
    create_trace, end_span, end_trace, finish_generation_span, finish_tool_span, LangfuseSpan,
    LangfuseTrace, TraceStatus,
};
