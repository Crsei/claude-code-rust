//! Unified frontend output sink.
//!
//! All backend→frontend message writes flow through [`FrontendSink`].
//! The first version is a thin wrapper around stdout JSON-line writes;
//! a later iteration can evolve into a single-writer `mpsc` model.

use std::io::{self, Write};

use super::protocol::BackendMessage;

/// Single point of egress for [`BackendMessage`]s to the frontend process.
///
/// `Clone` is free — the struct is currently zero-sized.
#[derive(Clone)]
pub struct FrontendSink;

impl FrontendSink {
    /// Create a sink that writes JSON lines to stdout.
    pub fn stdout() -> Self {
        Self
    }

    /// Serialize and write a single message as a JSON line.
    pub fn send(&self, msg: &BackendMessage) -> io::Result<()> {
        let json =
            serde_json::to_string(msg).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        let mut stdout = io::stdout().lock();
        writeln!(stdout, "{}", json)?;
        stdout.flush()
    }

    /// Write multiple messages, holding the stdout lock for the entire batch.
    pub fn send_many(&self, msgs: impl IntoIterator<Item = BackendMessage>) -> io::Result<()> {
        let mut stdout = io::stdout().lock();
        for msg in msgs {
            let json = serde_json::to_string(&msg)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            writeln!(stdout, "{}", json)?;
        }
        stdout.flush()
    }
}
