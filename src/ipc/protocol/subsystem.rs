//! Subsystem protocol serialization tests.
//!
//! Verifies that subsystem-related [`BackendMessage`] and [`FrontendMessage`]
//! variants round-trip correctly through JSON.

#[cfg(test)]
mod tests {
    use crate::ipc::protocol::{BackendMessage, FrontendMessage};

    #[test]
    fn backend_lsp_event_serializes() {
        use crate::ipc::subsystem_events::LspEvent;
        let msg = BackendMessage::LspEvent {
            event: LspEvent::ServerStateChanged {
                language_id: "rust".to_string(),
                state: "running".to_string(),
                error: None,
            },
        };
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["type"], "lsp_event");
        assert_eq!(json["event"]["kind"], "server_state_changed");
    }

    #[test]
    fn backend_subsystem_status_serializes() {
        use crate::ipc::subsystem_types::SubsystemStatusSnapshot;
        let msg = BackendMessage::SubsystemStatus {
            status: SubsystemStatusSnapshot {
                lsp: vec![],
                mcp: vec![],
                plugins: vec![],
                skills: vec![],
                timestamp: 100,
            },
        };
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["type"], "subsystem_status");
        assert_eq!(json["status"]["timestamp"], 100);
    }

    #[test]
    fn frontend_lsp_command_deserializes() {
        let json =
            r#"{"type":"lsp_command","command":{"kind":"start_server","language_id":"rust"}}"#;
        let msg: FrontendMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(msg, FrontendMessage::LspCommand { .. }));
    }

    #[test]
    fn frontend_query_subsystem_status_deserializes() {
        let json = r#"{"type":"query_subsystem_status"}"#;
        let msg: FrontendMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(msg, FrontendMessage::QuerySubsystemStatus));
    }
}
