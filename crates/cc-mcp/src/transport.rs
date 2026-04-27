//! MCP stdio transport -- background reader loop and response dispatch.
//!
//! The reader task reads line-delimited JSON-RPC messages from the server's
//! stdout and dispatches responses to waiting request futures via oneshot channels.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use serde_json::Value;

type PendingRequest = oneshot::Sender<Result<Value>>;
type PendingRequests = Arc<Mutex<HashMap<u64, PendingRequest>>>;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::{oneshot, Mutex};
use tracing::{debug, info, warn};

use super::JsonRpcResponse;

/// Background task that reads JSON-RPC responses from the MCP server's stdout.
///
/// Each line is parsed as a JSON-RPC response and dispatched to the
/// corresponding pending request via its oneshot channel.
pub(crate) async fn reader_loop(
    stdout: tokio::process::ChildStdout,
    pending: Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value>>>>>,
    server_name: String,
) {
    let reader = BufReader::new(stdout);
    let mut lines = reader.lines();

    loop {
        match lines.next_line().await {
            Ok(Some(line)) => {
                let line = line.trim().to_string();
                if line.is_empty() {
                    continue;
                }

                // Try to parse as a JSON-RPC response
                match serde_json::from_str::<JsonRpcResponse>(&line) {
                    Ok(response) => {
                        dispatch_response(&pending, &server_name, response).await;
                    }
                    Err(_) => {
                        // Could be a notification from the server
                        match serde_json::from_str::<Value>(&line) {
                            Ok(val) => {
                                if val.get("id").is_some() {
                                    warn!(
                                        server = %server_name,
                                        line = %line,
                                        "MCP: received malformed response"
                                    );
                                } else if let Some(method) =
                                    val.get("method").and_then(|m| m.as_str())
                                {
                                    debug!(
                                        server = %server_name,
                                        method = method,
                                        "MCP: received server notification"
                                    );
                                } else {
                                    debug!(
                                        server = %server_name,
                                        "MCP: received unknown JSON message"
                                    );
                                }
                            }
                            Err(e) => {
                                debug!(
                                    server = %server_name,
                                    error = %e,
                                    line = %line,
                                    "MCP: non-JSON line from server stdout"
                                );
                            }
                        }
                    }
                }
            }
            Ok(None) => {
                info!(server = %server_name, "MCP: server stdout closed (EOF)");
                break;
            }
            Err(e) => {
                warn!(
                    server = %server_name,
                    error = %e,
                    "MCP: error reading server stdout"
                );
                break;
            }
        }
    }

    // On exit, fail all pending requests
    let mut pending = pending.lock().await;
    for (id, sender) in pending.drain() {
        debug!(server = %server_name, id = id, "MCP: failing pending request (reader exited)");
        let _ = sender.send(Err(anyhow::anyhow!(
            "MCP server '{}' closed connection",
            server_name
        )));
    }
}

/// Dispatch a parsed JSON-RPC response to the corresponding pending request.
pub(crate) async fn dispatch_response(
    pending: &PendingRequests,
    server_name: &str,
    response: JsonRpcResponse,
) {
    let id = match response.id.as_u64() {
        Some(id) => id,
        None => {
            debug!(
                server = %server_name,
                id = ?response.id,
                "MCP: response has non-integer id, ignoring"
            );
            return;
        }
    };

    let mut pending = pending.lock().await;
    if let Some(sender) = pending.remove(&id) {
        let result = if let Some(error) = response.error {
            Err(anyhow::anyhow!(
                "MCP server '{}' returned error (code {}): {}",
                server_name,
                error.code,
                error.message
            ))
        } else {
            Ok(response.result.unwrap_or(Value::Null))
        };

        let _ = sender.send(result);
    } else {
        debug!(
            server = %server_name,
            id = id,
            "MCP: received response for unknown request id"
        );
    }
}
