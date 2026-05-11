//! Compatibility re-export for IPC protocol DTOs.

pub use cc_ipc_protocol::protocol::*;

#[cfg(test)]
mod tests {
    use super::BackendMessage;

    #[test]
    fn tombstone_reexport_serializes() {
        let msg = BackendMessage::Tombstone {
            message_id: "assistant-1".to_string(),
        };
        let value = serde_json::to_value(msg).expect("serialize tombstone");

        assert_eq!(value["type"], "tombstone");
        assert_eq!(value["message_id"], "assistant-1");
    }
}
