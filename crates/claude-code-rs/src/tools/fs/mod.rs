//! Filesystem tools sub-domain.
//!
//! Tools in this module read, write, search, or otherwise manipulate files and
//! directories on the local filesystem. They share concurrency-safety traits
//! (most are read-only and parallelizable) and live here so filesystem concerns
//! stay isolated from execution, code intelligence, or networking tools.
//!
//! See `src/tools/ARCHITECTURE.md` for placement rules.

use std::sync::Arc;

use crate::types::tool::Tools;

pub mod file_edit;
pub mod file_read;
pub mod file_write;
pub mod glob_tool;
pub mod grep;

/// Returns every tool owned by the filesystem sub-domain.
///
/// The registry aggregates each sub-domain's `tools()` instead of hard-coding
/// the full list, so adding a new fs tool only requires touching this file.
pub fn tools() -> Tools {
    vec![
        Arc::new(file_read::FileReadTool::new()),
        Arc::new(file_write::FileWriteTool::new()),
        Arc::new(file_edit::FileEditTool::new()),
        Arc::new(glob_tool::GlobTool::new()),
        Arc::new(grep::GrepTool),
    ]
}
