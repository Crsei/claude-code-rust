use gateway::adapters::telegram::{TelegramAdapter, TelegramTransport};
use gateway::{
    AdapterProvider, AdapterRegistry, AdapterState, AdapterTestMessage, GatewayError, RemoteAdapter,
};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct RecordingTelegramTransport {
    calls: Arc<Mutex<Vec<(String, String, Value)>>>,
    responses: Arc<Mutex<Vec<Value>>>,
}

impl RecordingTelegramTransport {
    fn new(responses: Vec<Value>) -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            responses: Arc::new(Mutex::new(responses)),
        }
    }

    fn calls(&self) -> Vec<(String, String, Value)> {
        self.calls.lock().unwrap().clone()
    }
}

impl TelegramTransport for RecordingTelegramTransport {
    fn call(&self, token: &str, method: &str, body: Value) -> Result<Value, GatewayError> {
        self.calls
            .lock()
            .unwrap()
            .push((token.to_string(), method.to_string(), body));
        Ok(self.responses.lock().unwrap().remove(0))
    }
}

fn telegram_config(token: Option<&str>) -> gateway::config::TelegramAdapterConfig {
    gateway::config::TelegramAdapterConfig {
        enabled: true,
        bot_token: token.map(str::to_string),
        test_chat_allowlist: vec!["chat-1".to_string()],
        ..Default::default()
    }
}

#[test]
fn registry_reports_registered_adapter_status() {
    let mut registry = AdapterRegistry::new();
    registry.register(TelegramAdapter::with_transport(
        telegram_config(None),
        RecordingTelegramTransport::new(vec![]),
    ));

    let statuses = registry.statuses();
    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].provider, AdapterProvider::Telegram);
    assert_eq!(statuses[0].state, AdapterState::Unconfigured);
}

#[test]
fn telegram_connect_uses_get_me_without_leaking_token() {
    let transport = RecordingTelegramTransport::new(vec![json!({ "ok": true })]);
    let adapter = TelegramAdapter::with_transport(
        telegram_config(Some("123456:raw-secret-token")),
        transport.clone(),
    );

    let status = adapter.connect().unwrap();

    assert_eq!(status.state, AdapterState::ConnectedOutboundOnly);
    let calls = transport.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].1, "getMe");
    let json = serde_json::to_string(&status).unwrap();
    assert!(!json.contains("raw-secret-token"));
    assert!(!json.contains("123456:"));
}

#[test]
fn telegram_test_message_enforces_allowlist() {
    let transport = RecordingTelegramTransport::new(vec![]);
    let adapter = TelegramAdapter::with_transport(
        telegram_config(Some("123456:raw-secret-token")),
        transport,
    );

    let err = adapter
        .test_message(AdapterTestMessage {
            target: "chat-2".to_string(),
            text: "hello".to_string(),
        })
        .unwrap_err();

    assert_eq!(err.diagnostic().code, "telegram_target_blocked");
    let json = serde_json::to_string(err.diagnostic()).unwrap();
    assert!(!json.contains("raw-secret-token"));
    assert!(!json.contains("123456:"));
}

#[test]
fn telegram_test_message_sends_only_outbound_message() {
    let transport = RecordingTelegramTransport::new(vec![json!({ "ok": true })]);
    let adapter = TelegramAdapter::with_transport(
        telegram_config(Some("123456:raw-secret-token")),
        transport.clone(),
    );

    let status = adapter
        .test_message(AdapterTestMessage {
            target: "chat-1".to_string(),
            text: "gateway smoke".to_string(),
        })
        .unwrap();

    assert_eq!(status.state, AdapterState::ConnectedOutboundOnly);
    let calls = transport.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].1, "sendMessage");
    assert_eq!(calls[0].2["chat_id"], "chat-1");
    assert_eq!(calls[0].2["text"], "gateway smoke");
}

#[test]
fn telegram_api_failure_diagnostic_is_redacted() {
    let transport = RecordingTelegramTransport::new(vec![json!({
        "ok": false,
        "error_code": 401,
        "description": "Unauthorized"
    })]);
    let adapter = TelegramAdapter::with_transport(
        telegram_config(Some("123456:raw-secret-token")),
        transport,
    );

    let err = adapter.connect().unwrap_err();
    let diagnostic = err.diagnostic();

    assert_eq!(diagnostic.code, "telegram_api_rejected");
    let json = serde_json::to_string(diagnostic).unwrap();
    assert!(json.contains("telegram_error_code=401"));
    assert!(!json.contains("raw-secret-token"));
    assert!(!json.contains("123456:"));
}
