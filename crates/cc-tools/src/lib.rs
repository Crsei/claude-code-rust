//! cc-tools: shared tool specs and pure registry helpers.
//!
//! This crate intentionally stays thin during the workspace split. Runtime
//! handlers remain with their owning domains until their dependencies can move
//! cleanly; this crate owns stable schemas, prompts, and selection helpers that
//! do not require engine, query, UI, IPC, or daemon state.

pub mod registry;
pub mod result;
pub mod task_specs;
