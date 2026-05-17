use gateway::adapters::lark::{LarkAdapter, LarkTransport};
use gateway::adapters::telegram::{TelegramAdapter, TelegramTransport};
use gateway::{
    AdapterProvider, AdapterRegistry, AdapterState, AdapterTestMessage, GatewayError, RemoteAdapter,
};
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

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

struct FakeHttpServer {
    base_url: String,
    requests: Arc<Mutex<Vec<String>>>,
    handle: Option<thread::JoinHandle<()>>,
}

impl FakeHttpServer {
    fn start(responses: Vec<Value>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let captured = requests.clone();
        let handle = thread::spawn(move || {
            for response in responses {
                let deadline = Instant::now() + Duration::from_secs(5);
                let mut stream = loop {
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                            if Instant::now() >= deadline {
                                return;
                            }
                            thread::sleep(Duration::from_millis(10));
                        }
                        Err(_) => return,
                    }
                };
                let mut buf = [0_u8; 8192];
                let len = stream.read(&mut buf).unwrap_or(0);
                captured
                    .lock()
                    .unwrap()
                    .push(String::from_utf8_lossy(&buf[..len]).to_string());
                let body = serde_json::to_string(&response).unwrap();
                let http = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                stream.write_all(http.as_bytes()).unwrap();
            }
        });
        Self {
            base_url,
            requests,
            handle: Some(handle),
        }
    }

    fn requests(&self) -> Vec<String> {
        self.requests.lock().unwrap().clone()
    }
}

impl Drop for FakeHttpServer {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.join().unwrap();
        }
    }
}

#[derive(Clone)]
struct RecordingLarkTransport {
    calls: Arc<Mutex<Vec<(String, Value)>>>,
    token_responses: Arc<Mutex<Vec<Value>>>,
}

impl RecordingLarkTransport {
    fn new(token_responses: Vec<Value>) -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            token_responses: Arc::new(Mutex::new(token_responses)),
        }
    }

    fn calls(&self) -> Vec<(String, Value)> {
        self.calls.lock().unwrap().clone()
    }
}

impl LarkTransport for RecordingLarkTransport {
    fn tenant_access_token(&self, app_id: &str, app_secret: &str) -> Result<Value, GatewayError> {
        self.calls.lock().unwrap().push((
            "tenant_access_token".to_string(),
            json!({ "app_id": app_id, "app_secret": app_secret }),
        ));
        Ok(self.token_responses.lock().unwrap().remove(0))
    }

    fn probe_webhook(&self, webhook_url: &str) -> Result<(), GatewayError> {
        self.calls.lock().unwrap().push((
            "probe_webhook".to_string(),
            json!({ "webhook_url": webhook_url }),
        ));
        Ok(())
    }

    fn send_webhook(&self, webhook_url: &str, text: &str) -> Result<(), GatewayError> {
        self.calls.lock().unwrap().push((
            "send_webhook".to_string(),
            json!({ "webhook_url": webhook_url, "text": text }),
        ));
        Ok(())
    }

    fn send_app_message(
        &self,
        tenant_access_token: &str,
        target: &str,
        text: &str,
    ) -> Result<(), GatewayError> {
        self.calls.lock().unwrap().push((
            "send_app_message".to_string(),
            json!({ "tenant_access_token": tenant_access_token, "target": target, "text": text }),
        ));
        Ok(())
    }
}

fn lark_config_with_app_credentials() -> gateway::config::LarkAdapterConfig {
    gateway::config::LarkAdapterConfig {
        enabled: true,
        app_id: Some("cli_a_raw_app_id".to_string()),
        app_secret: Some("raw-lark-secret".to_string()),
        test_target_allowlist: vec!["chat-1".to_string()],
        ..Default::default()
    }
}

fn lark_config_with_webhook() -> gateway::config::LarkAdapterConfig {
    gateway::config::LarkAdapterConfig {
        enabled: true,
        outbound_webhook_url: Some(
            "https://open.larksuite.com/open-apis/bot/v2/hook/raw-webhook-secret".to_string(),
        ),
        test_target_allowlist: vec!["hook-1".to_string()],
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
    registry.register(LarkAdapter::with_transport(
        gateway::config::LarkAdapterConfig {
            enabled: true,
            ..Default::default()
        },
        RecordingLarkTransport::new(vec![]),
    ));

    let statuses = registry.statuses();
    assert_eq!(statuses.len(), 2);
    assert!(statuses
        .iter()
        .any(|status| status.provider == AdapterProvider::Telegram
            && status.state == AdapterState::Unconfigured));
    assert!(statuses
        .iter()
        .any(|status| status.provider == AdapterProvider::Lark
            && status.state == AdapterState::Blocked));
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

#[test]
fn telegram_default_http_transport_uses_bot_api() {
    let server = FakeHttpServer::start(vec![json!({ "ok": true })]);
    let adapter = TelegramAdapter::new(gateway::config::TelegramAdapterConfig {
        enabled: true,
        api_base_url: server.base_url.clone(),
        bot_token: Some("123456:raw-secret-token".to_string()),
        ..telegram_config(Some("123456:raw-secret-token"))
    });

    let status = adapter.connect().unwrap();

    assert_eq!(status.state, AdapterState::ConnectedOutboundOnly);
    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].starts_with("POST /bot123456:raw-secret-token/getMe "));
    let json = serde_json::to_string(&status).unwrap();
    assert!(!json.contains("raw-secret-token"));
}

#[test]
fn lark_missing_credentials_returns_blocked_reason() {
    let adapter = LarkAdapter::with_transport(
        gateway::config::LarkAdapterConfig {
            enabled: true,
            ..Default::default()
        },
        RecordingLarkTransport::new(vec![]),
    );

    let status = adapter.status();
    assert_eq!(status.provider, AdapterProvider::Lark);
    assert_eq!(status.state, AdapterState::Blocked);
    assert_eq!(
        status.diagnostic.as_ref().unwrap().code,
        "lark_credentials_missing"
    );

    let err = adapter.connect().unwrap_err();
    assert_eq!(err.diagnostic().code, "lark_credentials_missing");
}

#[test]
fn lark_connect_with_app_credentials_is_outbound_only_and_redacted() {
    let transport = RecordingLarkTransport::new(vec![json!({
        "code": 0,
        "tenant_access_token": "tenant-raw-token"
    })]);
    let adapter =
        LarkAdapter::with_transport(lark_config_with_app_credentials(), transport.clone());

    let status = adapter.connect().unwrap();

    assert_eq!(status.state, AdapterState::ConnectedOutboundOnly);
    let calls = transport.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, "tenant_access_token");
    let json = serde_json::to_string(&status).unwrap();
    assert!(json.contains("outbound"));
    assert!(!json.contains("raw-lark-secret"));
    assert!(!json.contains("tenant-raw-token"));
    assert!(!json.contains("cli_a_raw_app_id"));
}

#[test]
fn lark_webhook_connect_is_outbound_only_without_inbound_claim() {
    let transport = RecordingLarkTransport::new(vec![]);
    let adapter = LarkAdapter::with_transport(lark_config_with_webhook(), transport.clone());

    let status = adapter.connect().unwrap();

    assert_eq!(status.state, AdapterState::ConnectedOutboundOnly);
    assert!(status
        .message
        .contains("inbound event control is not enabled"));
    let calls = transport.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, "probe_webhook");
    let json = serde_json::to_string(&status).unwrap();
    assert!(!json.contains("raw-webhook-secret"));
}

#[test]
fn lark_test_message_enforces_allowlist() {
    let adapter = LarkAdapter::with_transport(
        lark_config_with_webhook(),
        RecordingLarkTransport::new(vec![]),
    );

    let err = adapter
        .test_message(AdapterTestMessage {
            target: "hook-2".to_string(),
            text: "hello".to_string(),
        })
        .unwrap_err();

    assert_eq!(err.diagnostic().code, "lark_target_blocked");
    let json = serde_json::to_string(err.diagnostic()).unwrap();
    assert!(!json.contains("raw-webhook-secret"));
}

#[test]
fn lark_test_message_uses_webhook_outbound_path() {
    let transport = RecordingLarkTransport::new(vec![]);
    let adapter = LarkAdapter::with_transport(lark_config_with_webhook(), transport.clone());

    let status = adapter
        .test_message(AdapterTestMessage {
            target: "hook-1".to_string(),
            text: "gateway smoke".to_string(),
        })
        .unwrap();

    assert_eq!(status.state, AdapterState::ConnectedOutboundOnly);
    let calls = transport.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, "send_webhook");
    assert_eq!(calls[0].1["text"], "gateway smoke");
}

#[test]
fn lark_api_failure_diagnostic_is_redacted() {
    let transport = RecordingLarkTransport::new(vec![json!({
        "code": 99991663,
        "msg": "invalid app_secret"
    })]);
    let adapter = LarkAdapter::with_transport(lark_config_with_app_credentials(), transport);

    let err = adapter.connect().unwrap_err();
    let diagnostic = err.diagnostic();

    assert_eq!(diagnostic.code, "lark_api_rejected");
    let json = serde_json::to_string(diagnostic).unwrap();
    assert!(json.contains("lark_code=99991663"));
    assert!(!json.contains("raw-lark-secret"));
    assert!(!json.contains("cli_a_raw_app_id"));
}

#[test]
fn lark_default_http_transport_sends_app_message() {
    let server = FakeHttpServer::start(vec![
        json!({ "code": 0, "tenant_access_token": "tenant-raw-token" }),
        json!({ "code": 0 }),
    ]);
    let adapter = LarkAdapter::new(gateway::config::LarkAdapterConfig {
        enabled: true,
        api_base_url: server.base_url.clone(),
        app_id: Some("cli_a_raw_app_id".to_string()),
        app_secret: Some("raw-lark-secret".to_string()),
        test_target_allowlist: vec!["chat-1".to_string()],
        ..Default::default()
    });

    let status = adapter
        .test_message(AdapterTestMessage {
            target: "chat-1".to_string(),
            text: "gateway smoke".to_string(),
        })
        .unwrap();

    assert_eq!(status.state, AdapterState::ConnectedOutboundOnly);
    let requests = server.requests();
    assert_eq!(requests.len(), 2);
    assert!(requests[0].starts_with("POST /open-apis/auth/v3/tenant_access_token/internal "));
    assert!(requests[1].starts_with("POST /open-apis/im/v1/messages?receive_id_type=chat_id "));
    assert!(requests[1]
        .to_ascii_lowercase()
        .contains("authorization: bearer tenant-raw-token"));
    let json = serde_json::to_string(&status).unwrap();
    assert!(!json.contains("raw-lark-secret"));
    assert!(!json.contains("tenant-raw-token"));
}
