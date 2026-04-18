use super::super::{JsonRpcError, JsonRpcResponse, McpConnectionState, McpServerConfig};
use super::*;
use crate::mcp::manager::McpManager;
use crate::mcp::transport::dispatch_response;

use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use anyhow::Result;
use serde_json::{json, Value};
use tokio::sync::{oneshot, Mutex};

#[test]
fn test_mcp_client_new() {
    let config = McpServerConfig {
        name: "test-server".to_string(),
        transport: "stdio".to_string(),
        command: Some("echo".to_string()),
        args: Some(vec!["hello".to_string()]),
        url: None,
        headers: None,
        env: None,
        browser_mcp: None,
    };

    let client = McpClient::new(config);
    assert_eq!(client.state, McpConnectionState::Pending);
    assert!(client.tools.is_empty());
    assert!(client.resources.is_empty());
}

#[test]
fn test_mcp_manager_new() {
    let manager = McpManager::new();
    assert!(manager.clients.is_empty());
    assert!(manager.all_tools().is_empty());
    assert!(manager.all_resources().is_empty());
}

#[test]
fn test_jsonrpc_request_ids_increment() {
    let config = McpServerConfig {
        name: "test".to_string(),
        transport: "stdio".to_string(),
        command: Some("echo".to_string()),
        args: None,
        url: None,
        headers: None,
        env: None,
        browser_mcp: None,
    };
    let client = McpClient::new(config);

    let id1 = client.next_id.fetch_add(1, Ordering::SeqCst);
    let id2 = client.next_id.fetch_add(1, Ordering::SeqCst);
    assert_eq!(id1 + 1, id2);
}

#[tokio::test]
async fn test_dispatch_response_success() {
    let pending: Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value>>>>> =
        Arc::new(Mutex::new(HashMap::new()));

    let (tx, rx) = oneshot::channel();
    {
        let mut p = pending.lock().await;
        p.insert(42, tx);
    }

    let response = JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id: json!(42),
        result: Some(json!({"tools": []})),
        error: None,
    };

    dispatch_response(&pending, "test", response).await;

    let result = rx.await.unwrap().unwrap();
    assert_eq!(result, json!({"tools": []}));
}

#[tokio::test]
async fn test_dispatch_response_error() {
    let pending: Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value>>>>> =
        Arc::new(Mutex::new(HashMap::new()));

    let (tx, rx) = oneshot::channel();
    {
        let mut p = pending.lock().await;
        p.insert(7, tx);
    }

    let response = JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id: json!(7),
        result: None,
        error: Some(JsonRpcError {
            code: -32600,
            message: "Invalid Request".to_string(),
            data: None,
        }),
    };

    dispatch_response(&pending, "test", response).await;

    let result = rx.await.unwrap();
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Invalid Request"));
}

#[tokio::test]
async fn test_dispatch_response_unknown_id() {
    let pending: Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value>>>>> =
        Arc::new(Mutex::new(HashMap::new()));

    let response = JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id: json!(99),
        result: Some(json!("ignored")),
        error: None,
    };

    // Should not panic
    dispatch_response(&pending, "test", response).await;
}

#[tokio::test]
async fn test_connect_stdio_missing_command() {
    let config = McpServerConfig {
        name: "bad-server".to_string(),
        transport: "stdio".to_string(),
        command: None,
        args: None,
        url: None,
        headers: None,
        env: None,
        browser_mcp: None,
    };

    let mut client = McpClient::new(config);
    let result = client.connect().await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("command"));
}

#[tokio::test]
async fn test_connect_sse_not_implemented() {
    let config = McpServerConfig {
        name: "sse-server".to_string(),
        transport: "sse".to_string(),
        command: None,
        args: None,
        url: Some("http://localhost:8080".to_string()),
        headers: None,
        env: None,
        browser_mcp: None,
    };

    let mut client = McpClient::new(config);
    let result = client.connect().await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("SSE"));
}

#[tokio::test]
async fn test_disconnect_idempotent() {
    let config = McpServerConfig {
        name: "test".to_string(),
        transport: "stdio".to_string(),
        command: Some("echo".to_string()),
        args: None,
        url: None,
        headers: None,
        env: None,
        browser_mcp: None,
    };

    let mut client = McpClient::new(config);
    client.disconnect().await;
    assert_eq!(client.state, McpConnectionState::Disconnected);

    client.disconnect().await;
    assert_eq!(client.state, McpConnectionState::Disconnected);
}

#[tokio::test]
async fn test_list_tools_not_connected() {
    let config = McpServerConfig {
        name: "test".to_string(),
        transport: "stdio".to_string(),
        command: Some("echo".to_string()),
        args: None,
        url: None,
        headers: None,
        env: None,
        browser_mcp: None,
    };

    let mut client = McpClient::new(config);
    let result = client.list_tools().await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not connected"));
}

#[tokio::test]
async fn test_call_tool_not_connected() {
    let config = McpServerConfig {
        name: "test".to_string(),
        transport: "stdio".to_string(),
        command: Some("echo".to_string()),
        args: None,
        url: None,
        headers: None,
        env: None,
        browser_mcp: None,
    };

    let client = McpClient::new(config);
    let result = client.call_tool("test", json!({})).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_mcp_manager_connect_all_invalid_server() {
    let mut manager = McpManager::new();
    let configs = vec![McpServerConfig {
        name: "nonexistent".to_string(),
        transport: "stdio".to_string(),
        command: Some("this-command-does-not-exist-at-all-12345".to_string()),
        args: None,
        url: None,
        headers: None,
        env: None,
        browser_mcp: None,
    }];

    let result = manager.connect_all(configs).await;
    assert!(result.is_ok());
    assert!(manager.clients.is_empty());
}
