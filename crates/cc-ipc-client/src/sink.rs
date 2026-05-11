//! Frontend output sink for IPC backend messages.

use std::io::{self, Write};
use std::sync::Arc;

use cc_ipc_protocol::BackendMessage;
use parking_lot::Mutex;

trait SinkWriter: Send + Sync {
    fn write_one(&self, msg: &BackendMessage) -> io::Result<()>;
    fn write_many(&self, msgs: Vec<BackendMessage>) -> io::Result<()>;
}

struct StdoutWriter;

impl SinkWriter for StdoutWriter {
    fn write_one(&self, msg: &BackendMessage) -> io::Result<()> {
        let json = serde_json::to_string(msg).map_err(io::Error::other)?;
        let mut stdout = io::stdout().lock();
        writeln!(stdout, "{}", json)?;
        stdout.flush()
    }

    fn write_many(&self, msgs: Vec<BackendMessage>) -> io::Result<()> {
        let mut stdout = io::stdout().lock();
        for msg in msgs {
            let json = serde_json::to_string(&msg).map_err(io::Error::other)?;
            writeln!(stdout, "{}", json)?;
        }
        stdout.flush()
    }
}

#[derive(Default)]
struct MemoryWriter {
    messages: Mutex<Vec<BackendMessage>>,
}

impl SinkWriter for MemoryWriter {
    fn write_one(&self, msg: &BackendMessage) -> io::Result<()> {
        self.messages.lock().push(msg.clone());
        Ok(())
    }

    fn write_many(&self, msgs: Vec<BackendMessage>) -> io::Result<()> {
        self.messages.lock().extend(msgs);
        Ok(())
    }
}

/// Single point of egress for [`BackendMessage`]s to a frontend process.
#[derive(Clone)]
pub struct FrontendSink {
    writer: Arc<dyn SinkWriter>,
    memory: Option<Arc<MemoryWriter>>,
}

impl FrontendSink {
    /// Create a sink that writes JSON lines to stdout.
    pub fn stdout() -> Self {
        Self {
            writer: Arc::new(StdoutWriter),
            memory: None,
        }
    }

    /// Create an in-memory sink for tests and runtime adapters.
    pub fn memory() -> Self {
        let memory = Arc::new(MemoryWriter::default());
        Self {
            writer: memory.clone(),
            memory: Some(memory),
        }
    }

    /// Serialize and write a single message.
    pub fn send(&self, msg: &BackendMessage) -> io::Result<()> {
        self.writer.write_one(msg)
    }

    /// Write multiple messages as one batch.
    pub fn send_many(&self, msgs: impl IntoIterator<Item = BackendMessage>) -> io::Result<()> {
        self.writer.write_many(msgs.into_iter().collect())
    }

    /// Return messages captured by an in-memory sink.
    pub fn captured(&self) -> Vec<BackendMessage> {
        self.memory
            .as_ref()
            .map(|memory| memory.messages.lock().clone())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_sink_captures_single_and_batch_messages() {
        let sink = FrontendSink::memory();
        sink.send(&BackendMessage::SystemInfo {
            text: "one".to_string(),
            level: "info".to_string(),
        })
        .unwrap();
        sink.send_many([BackendMessage::Error {
            message: "two".to_string(),
            recoverable: true,
        }])
        .unwrap();

        assert_eq!(sink.captured().len(), 2);
    }
}
