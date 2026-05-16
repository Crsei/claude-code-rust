//! Compatibility re-export for JSONL transport helpers.

pub use cc_ipc_transport::{
    parse_frontend_line, JsonlStdioReader, JsonlStdioTransport, JsonlStdioWriter,
    ParsedFrontendLine,
};
