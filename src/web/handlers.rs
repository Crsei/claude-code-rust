//! Axum route handlers for the web chat API.

use std::sync::atomic::Ordering;

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::engine::sdk_types::SdkMessage;
use crate::types::config::QuerySource;
use crate::types::tool::PermissionMode;

use super::sse::sdk_stream_to_sse;
use super::state::WebState;

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct ChatRequest {
    pub message: String,
    #[serde(default)]
    pub session_id: Option<String>,
}

#[derive(Deserialize)]
pub struct AbortRequest {
    #[serde(default)]
    pub session_id: Option<String>,
}

#[derive(Serialize)]
pub struct ApiError {
    pub error: String,
    pub code: String,
}

#[derive(Serialize)]
pub struct StateResponse {
    pub model: String,
    pub session_id: String,
    pub tools: Vec<String>,
    pub permission_mode: String,
    pub thinking_enabled: Option<bool>,
    pub fast_mode: bool,
    pub effort: Option<String>,
    // Phase 3 additions
    pub usage: UsageResponse,
    pub commands: Vec<CommandInfo>,
}

#[derive(Serialize)]
pub struct UsageResponse {
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_cache_read_tokens: u64,
    pub total_cache_creation_tokens: u64,
    pub total_cost_usd: f64,
    pub api_call_count: u64,
}

#[derive(Serialize, Clone)]
pub struct CommandInfo {
    pub name: String,
    pub aliases: Vec<String>,
    pub description: String,
}

// Phase 3: Settings mutation
#[derive(Deserialize)]
pub struct SettingsRequest {
    pub action: String,
    pub value: serde_json::Value,
}

#[derive(Serialize)]
pub struct SettingsResponse {
    pub ok: bool,
    pub message: String,
}

// Phase 3: Command execution
#[derive(Deserialize)]
pub struct CommandRequest {
    pub command: String,
    #[serde(default)]
    pub args: String,
}

#[derive(Serialize)]
pub struct CommandResponse {
    #[serde(rename = "type")]
    pub response_type: String, // "output" | "clear" | "error"
    pub content: String,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// POST /api/chat -- Start a streaming chat response via SSE.
pub async fn chat_handler(
    State(state): State<WebState>,
    Json(req): Json<ChatRequest>,
) -> impl IntoResponse {
    // Check if already streaming
    if state.is_streaming.load(Ordering::SeqCst) {
        return (
            StatusCode::CONFLICT,
            Json(ApiError {
                error: "A query is already in progress".into(),
                code: "engine_busy".into(),
            }),
        )
            .into_response();
    }

    info!(message = %req.message, "POST /api/chat");

    state.is_streaming.store(true, Ordering::SeqCst);

    // Get the stream from the engine
    let stream = state.engine.submit_message(&req.message, QuerySource::Sdk);

    // Wrap in a stream that clears is_streaming when done
    let is_streaming = state.is_streaming.clone();
    let wrapped_stream = Box::pin(futures::stream::unfold(
        (stream, is_streaming, false),
        |(mut stream, flag, done)| async move {
            if done {
                return None;
            }
            use futures::StreamExt;
            match stream.next().await {
                Some(msg) => {
                    let is_result = matches!(&msg, SdkMessage::Result(_));
                    if is_result {
                        flag.store(false, Ordering::SeqCst);
                    }
                    Some((msg, (stream, flag, is_result)))
                }
                None => {
                    flag.store(false, Ordering::SeqCst);
                    None
                }
            }
        },
    ));

    sdk_stream_to_sse(wrapped_stream).into_response()
}

/// POST /api/abort -- Abort the current generation.
pub async fn abort_handler(
    State(state): State<WebState>,
    Json(_req): Json<AbortRequest>,
) -> impl IntoResponse {
    info!("POST /api/abort");
    state.engine.abort();
    state.is_streaming.store(false, Ordering::SeqCst);
    StatusCode::OK
}

/// GET /api/state -- Return current application state (enhanced for Phase 3).
pub async fn state_handler(State(state): State<WebState>) -> impl IntoResponse {
    let app_state = state.engine.app_state();
    let permission_mode = app_state.tool_permission_context.mode.as_str();

    // Get tool names from engine
    let tool_names: Vec<String> = state.engine.tool_names();

    // Get usage tracking
    let usage = state.engine.usage();

    // Get command list
    let commands: Vec<CommandInfo> = crate::commands::get_all_commands()
        .iter()
        .map(|c| CommandInfo {
            name: c.name.clone(),
            aliases: c.aliases.clone(),
            description: c.description.clone(),
        })
        .collect();

    Json(StateResponse {
        model: app_state.main_loop_model.clone(),
        session_id: state.engine.session_id.to_string(),
        tools: tool_names,
        permission_mode: permission_mode.to_string(),
        thinking_enabled: app_state.thinking_enabled,
        fast_mode: app_state.fast_mode,
        effort: app_state.effort_value.clone(),
        usage: UsageResponse {
            total_input_tokens: usage.total_input_tokens,
            total_output_tokens: usage.total_output_tokens,
            total_cache_read_tokens: usage.total_cache_read_tokens,
            total_cache_creation_tokens: usage.total_cache_creation_tokens,
            total_cost_usd: usage.total_cost_usd,
            api_call_count: usage.api_call_count,
        },
        commands,
    })
}

/// POST /api/settings -- Mutate application settings.
pub async fn settings_handler(
    State(state): State<WebState>,
    Json(req): Json<SettingsRequest>,
) -> impl IntoResponse {
    info!(action = %req.action, "POST /api/settings");

    match req.action.as_str() {
        "set_model" => {
            let model = req.value.as_str().unwrap_or("").to_string();
            if model.is_empty() {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(SettingsResponse {
                        ok: false,
                        message: "model name required".into(),
                    }),
                );
            }
            // Resolve model aliases
            let resolved = resolve_model_alias(&model);
            state.engine.update_app_state(|s| {
                s.main_loop_model = resolved.clone();
                s.settings.model = Some(resolved.clone());
            });
            (
                StatusCode::OK,
                Json(SettingsResponse {
                    ok: true,
                    message: format!("Model set to {}", resolved),
                }),
            )
        }
        "set_permission_mode" => {
            let mode_str = req.value.as_str().unwrap_or("default");
            let mode = match mode_str {
                "auto" => PermissionMode::Auto,
                "bypass" => PermissionMode::Bypass,
                "plan" => PermissionMode::Plan,
                _ => PermissionMode::Default,
            };
            state.engine.update_app_state(|s| {
                s.tool_permission_context.mode = mode.clone();
            });
            (
                StatusCode::OK,
                Json(SettingsResponse {
                    ok: true,
                    message: format!("Permission mode set to {}", mode_str),
                }),
            )
        }
        "set_thinking" => {
            let enabled = req.value.as_bool();
            state.engine.update_app_state(|s| {
                s.thinking_enabled = enabled;
            });
            (
                StatusCode::OK,
                Json(SettingsResponse {
                    ok: true,
                    message: format!("Thinking set to {:?}", enabled),
                }),
            )
        }
        "set_fast_mode" => {
            let enabled = req.value.as_bool().unwrap_or(false);
            state.engine.update_app_state(|s| {
                s.fast_mode = enabled;
            });
            (
                StatusCode::OK,
                Json(SettingsResponse {
                    ok: true,
                    message: format!("Fast mode {}", if enabled { "enabled" } else { "disabled" }),
                }),
            )
        }
        "set_effort" => {
            let effort = req.value.as_str().map(|s| s.to_string());
            state.engine.update_app_state(|s| {
                s.effort_value = effort.clone();
            });
            (
                StatusCode::OK,
                Json(SettingsResponse {
                    ok: true,
                    message: format!("Effort set to {:?}", effort),
                }),
            )
        }
        _ => (
            StatusCode::BAD_REQUEST,
            Json(SettingsResponse {
                ok: false,
                message: format!("Unknown action: {}", req.action),
            }),
        ),
    }
}

/// POST /api/command -- Execute a slash command.
pub async fn command_handler(
    State(state): State<WebState>,
    Json(req): Json<CommandRequest>,
) -> impl IntoResponse {
    info!(command = %req.command, args = %req.args, "POST /api/command");

    let commands = crate::commands::get_all_commands();
    let cmd = commands
        .iter()
        .find(|c| c.name == req.command || c.aliases.contains(&req.command));

    let cmd = match cmd {
        Some(c) => c,
        None => {
            return Json(CommandResponse {
                response_type: "error".into(),
                content: format!("Unknown command: /{}", req.command),
            });
        }
    };

    // Build a CommandContext
    let messages = state.engine.messages();
    let app_state = state.engine.app_state();
    let cwd = std::path::PathBuf::from(state.engine.cwd());

    let mut ctx = crate::commands::CommandContext {
        messages,
        cwd,
        app_state: app_state.clone(),
        session_id: state.engine.session_id.clone(),
    };

    match cmd.handler.execute(&req.args, &mut ctx).await {
        Ok(result) => {
            // Apply any state mutations from the command
            // Commands mutate ctx.app_state in-place; write it back
            state.engine.update_app_state(|s| {
                s.main_loop_model = ctx.app_state.main_loop_model.clone();
                s.settings = ctx.app_state.settings.clone();
                s.tool_permission_context = ctx.app_state.tool_permission_context.clone();
                s.thinking_enabled = ctx.app_state.thinking_enabled;
                s.fast_mode = ctx.app_state.fast_mode;
                s.effort_value = ctx.app_state.effort_value.clone();
            });

            match result {
                crate::commands::CommandResult::Output(text) => Json(CommandResponse {
                    response_type: "output".into(),
                    content: text,
                }),
                crate::commands::CommandResult::Clear => Json(CommandResponse {
                    response_type: "clear".into(),
                    content: "Conversation cleared".into(),
                }),
                crate::commands::CommandResult::Exit(msg) => Json(CommandResponse {
                    response_type: "output".into(),
                    content: msg,
                }),
                crate::commands::CommandResult::Query(_msgs) => {
                    // TODO: inject messages and start a new SSE stream
                    Json(CommandResponse {
                        response_type: "output".into(),
                        content: "Command queued (query commands not yet supported in web UI)"
                            .into(),
                    })
                }
                crate::commands::CommandResult::None => Json(CommandResponse {
                    response_type: "output".into(),
                    content: "OK".into(),
                }),
            }
        }
        Err(e) => Json(CommandResponse {
            response_type: "error".into(),
            content: format!("Command error: {}", e),
        }),
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Resolve common model aliases to full model names.
fn resolve_model_alias(name: &str) -> String {
    match name.to_lowercase().as_str() {
        "opus" | "claude-opus" => "claude-opus-4-20250514".to_string(),
        "sonnet" | "claude-sonnet" => "claude-sonnet-4-20250514".to_string(),
        "haiku" | "claude-haiku" => "claude-haiku-3-5-20241022".to_string(),
        "deepseek" | "deepseek-chat" => "deepseek-chat".to_string(),
        "deepseek-reasoner" | "r1" => "deepseek-reasoner".to_string(),
        "gpt-4o" | "4o" => "gpt-4o".to_string(),
        "gpt-4.1" => "gpt-4.1".to_string(),
        "o3" => "o3".to_string(),
        "o4-mini" => "o4-mini".to_string(),
        "gemini-2.5-pro" | "gemini-pro" => "gemini-2.5-pro-preview-05-06".to_string(),
        other => other.to_string(),
    }
}
