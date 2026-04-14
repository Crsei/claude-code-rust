use std::path::PathBuf;
use std::process::ExitStatus;

use anyhow::{Context, Result};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{Child, ChildStdout, Command};

use crate::types::message::{ContentBlock, Message, MessageContent, ToolResultContent};
use crate::types::tool::PermissionMode;

pub const NATIVE_BACKEND_NAME: &str = "native";
pub const CODEX_BACKEND_NAME: &str = "codex";
pub const DEFAULT_CODEX_MODEL: &str = "gpt-5.2-codex";

const MAX_PROMPT_CHARS: usize = 120_000;

pub fn normalize_backend(value: Option<&str>) -> String {
    let raw = value.unwrap_or(NATIVE_BACKEND_NAME).trim();
    if raw.eq_ignore_ascii_case(CODEX_BACKEND_NAME) {
        CODEX_BACKEND_NAME.to_string()
    } else {
        NATIVE_BACKEND_NAME.to_string()
    }
}

pub fn is_codex_backend(value: &str) -> bool {
    value.eq_ignore_ascii_case(CODEX_BACKEND_NAME)
}

#[derive(Debug, Clone)]
pub struct CodexExecRequest {
    pub prompt: String,
    pub model: String,
    pub cwd: String,
    pub permission_mode: PermissionMode,
}

pub struct CodexExecProcess {
    child: Child,
    stdout_lines: Lines<BufReader<ChildStdout>>,
    stderr_task: tokio::task::JoinHandle<Result<String>>,
}

#[derive(Debug)]
pub struct CodexExecCompletion {
    pub status: ExitStatus,
    pub stderr: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CodexUsage {
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodexExecEvent {
    ThreadStarted { thread_id: String },
    TurnStarted,
    ItemStarted(CodexExecItem),
    ItemCompleted(CodexExecItem),
    TurnCompleted { usage: CodexUsage },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodexExecItem {
    AgentMessage {
        id: String,
        text: String,
    },
    CommandExecution {
        id: String,
        command: String,
        aggregated_output: String,
        exit_code: Option<i32>,
        status: Option<String>,
    },
    Other {
        id: Option<String>,
        item_type: String,
    },
}

#[derive(Debug, serde::Deserialize)]
struct RawCodexEvent {
    #[serde(rename = "type")]
    event_type: String,
    #[serde(default)]
    thread_id: Option<String>,
    #[serde(default)]
    item: Option<RawCodexItem>,
    #[serde(default)]
    usage: Option<RawCodexUsage>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct RawCodexItem {
    id: String,
    #[serde(rename = "type")]
    item_type: String,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    command: Option<String>,
    #[serde(default)]
    aggregated_output: Option<String>,
    #[serde(default)]
    exit_code: Option<i32>,
    #[serde(default)]
    status: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct RawCodexUsage {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    cached_input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
}

impl TryFrom<RawCodexEvent> for CodexExecEvent {
    type Error = anyhow::Error;

    fn try_from(value: RawCodexEvent) -> Result<Self> {
        match value.event_type.as_str() {
            "thread.started" => Ok(Self::ThreadStarted {
                thread_id: value.thread_id.unwrap_or_default(),
            }),
            "turn.started" => Ok(Self::TurnStarted),
            "item.started" => Ok(Self::ItemStarted(raw_item_to_item(value.item)?)),
            "item.completed" => Ok(Self::ItemCompleted(raw_item_to_item(value.item)?)),
            "turn.completed" => {
                let usage = value.usage.unwrap_or(RawCodexUsage {
                    input_tokens: 0,
                    cached_input_tokens: 0,
                    output_tokens: 0,
                });
                Ok(Self::TurnCompleted {
                    usage: CodexUsage {
                        input_tokens: usage.input_tokens,
                        cached_input_tokens: usage.cached_input_tokens,
                        output_tokens: usage.output_tokens,
                    },
                })
            }
            other => anyhow::bail!("unsupported codex event type: {}", other),
        }
    }
}

fn raw_item_to_item(raw: Option<RawCodexItem>) -> Result<CodexExecItem> {
    let raw = raw.context("codex event missing item payload")?;
    match raw.item_type.as_str() {
        "agent_message" => Ok(CodexExecItem::AgentMessage {
            id: raw.id,
            text: raw.text.unwrap_or_default(),
        }),
        "command_execution" => Ok(CodexExecItem::CommandExecution {
            id: raw.id,
            command: raw.command.unwrap_or_default(),
            aggregated_output: raw.aggregated_output.unwrap_or_default(),
            exit_code: raw.exit_code,
            status: raw.status,
        }),
        other => Ok(CodexExecItem::Other {
            id: Some(raw.id),
            item_type: other.to_string(),
        }),
    }
}

pub async fn spawn_codex_exec(request: CodexExecRequest) -> Result<CodexExecProcess> {
    let program = find_codex_program().context("failed to locate a local codex executable")?;
    let mut command = Command::new(program);
    command
        .arg("exec")
        .arg("--json")
        .arg("--ephemeral")
        .arg("--skip-git-repo-check")
        .arg("--color")
        .arg("never")
        .arg("-m")
        .arg(&request.model)
        .arg("-C")
        .arg(&request.cwd)
        .arg("-");

    match request.permission_mode {
        PermissionMode::Bypass => {
            command.arg("--dangerously-bypass-approvals-and-sandbox");
        }
        PermissionMode::Auto => {
            command.arg("--full-auto");
        }
        PermissionMode::Plan | PermissionMode::Default => {
            command.arg("--sandbox").arg("read-only");
        }
    }

    command
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let mut child = command.spawn().context("failed to spawn codex exec")?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(request.prompt.as_bytes())
            .await
            .context("failed to write prompt to codex stdin")?;
        stdin
            .write_all(b"\n")
            .await
            .context("failed to terminate codex prompt with newline")?;
        stdin.shutdown().await.ok();
    }

    let stdout = child
        .stdout
        .take()
        .context("codex exec stdout was not piped")?;
    let stderr = child
        .stderr
        .take()
        .context("codex exec stderr was not piped")?;

    let stderr_task = tokio::spawn(async move {
        let mut reader = BufReader::new(stderr);
        let mut stderr_text = String::new();
        reader
            .read_to_string(&mut stderr_text)
            .await
            .context("failed to read codex stderr")?;
        Ok(stderr_text)
    });

    Ok(CodexExecProcess {
        child,
        stdout_lines: BufReader::new(stdout).lines(),
        stderr_task,
    })
}

impl CodexExecProcess {
    pub async fn next_event(&mut self) -> Result<Option<CodexExecEvent>> {
        while let Some(line) = self
            .stdout_lines
            .next_line()
            .await
            .context("failed to read codex stdout")?
        {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let raw: RawCodexEvent = serde_json::from_str(trimmed)
                .with_context(|| format!("failed to parse codex JSONL event: {}", trimmed))?;
            match CodexExecEvent::try_from(raw) {
                Ok(event) => return Ok(Some(event)),
                Err(_) => continue,
            }
        }
        Ok(None)
    }

    pub async fn kill(&mut self) -> Result<()> {
        self.child.kill().await.context("failed to kill codex exec")
    }

    pub async fn wait(mut self) -> Result<CodexExecCompletion> {
        let status = self.child.wait().await.context("failed waiting for codex exec")?;
        let stderr = self
            .stderr_task
            .await
            .context("codex stderr task join failed")??;
        Ok(CodexExecCompletion { status, stderr })
    }
}

pub fn build_codex_prompt(
    messages: &[Message],
    custom_system_prompt: Option<&str>,
    append_system_prompt: Option<&str>,
) -> String {
    let mut sections = vec![
        "Continue the following cc-rust conversation inside the current workspace using Codex."
            .to_string(),
        "Treat the latest user message as the active request. Preserve relevant context from earlier messages when answering.".to_string(),
    ];
    if cfg!(windows) {
        sections.push(
            "When you need shell commands on Windows, prefer PowerShell with `-NoProfile`, avoid profile-dependent startup behavior, and do not modify `[Console]::OutputEncoding` unless the task explicitly requires it."
                .to_string(),
        );
    }

    let mut extra_instructions = Vec::new();
    if let Some(prompt) = custom_system_prompt.filter(|value| !value.trim().is_empty()) {
        extra_instructions.push(prompt.trim().to_string());
    }
    if let Some(prompt) = append_system_prompt.filter(|value| !value.trim().is_empty()) {
        extra_instructions.push(prompt.trim().to_string());
    }
    if !extra_instructions.is_empty() {
        sections.push(format!(
            "Additional instructions:\n{}",
            extra_instructions.join("\n\n")
        ));
    }

    sections.push(format!(
        "Conversation transcript:\n{}",
        format_codex_conversation(messages)
    ));

    sections.join("\n\n")
}

pub fn sanitize_command_output(output: &str) -> String {
    let mut cleaned: Vec<String> = Vec::new();
    let mut lines = output.lines().peekable();

    while let Some(line) = lines.next() {
        if is_profile_noise_start(line) {
            skip_noise_block(&mut lines);
            continue;
        }

        if line.contains("[Console]::OutputEncoding=[System.Text.Encoding]::UTF8;") {
            while cleaned
                .last()
                .is_some_and(|previous| is_output_encoding_context_line(previous))
            {
                cleaned.pop();
            }
            skip_output_encoding_block(&mut lines);
            continue;
        }

        cleaned.push(line.to_string());
    }

    cleaned.join("\n").trim().to_string()
}

fn format_codex_conversation(messages: &[Message]) -> String {
    let mut parts = Vec::new();
    for message in messages {
        match message {
            Message::User(user) => {
                let rendered = match &user.content {
                    MessageContent::Text(text) => text.trim().to_string(),
                    MessageContent::Blocks(blocks) => render_blocks(blocks),
                };
                if !rendered.is_empty() {
                    parts.push(format!("User: {}", rendered));
                }
            }
            Message::Assistant(assistant) => {
                let rendered = render_blocks(&assistant.content);
                if !rendered.is_empty() {
                    parts.push(format!("Assistant: {}", rendered));
                }
            }
            Message::System(system) => {
                if !system.content.trim().is_empty() {
                    parts.push(format!("System: {}", system.content.trim()));
                }
            }
            _ => {}
        }
    }

    let joined = parts.join("\n\n");
    if joined.len() > MAX_PROMPT_CHARS {
        let start = joined.len().saturating_sub(MAX_PROMPT_CHARS);
        format!(
            "[Earlier conversation omitted]\n{}",
            &joined[start..]
        )
    } else if joined.is_empty() {
        "[No prior conversation]".to_string()
    } else {
        joined
    }
}

fn render_blocks(blocks: &[ContentBlock]) -> String {
    let mut rendered = Vec::new();
    for block in blocks {
        match block {
            ContentBlock::Text { text } => {
                if !text.trim().is_empty() {
                    rendered.push(text.trim().to_string());
                }
            }
            ContentBlock::ToolUse { name, input, .. } => {
                rendered.push(format!("[Tool use: {} {}]", name, input));
            }
            ContentBlock::ToolResult {
                tool_use_id,
                content,
                is_error,
            } => {
                let rendered_content = match content {
                    ToolResultContent::Text(text) => text.trim().to_string(),
                    ToolResultContent::Blocks(inner) => render_blocks(inner),
                };
                rendered.push(format!(
                    "[Tool result {}{}: {}]",
                    tool_use_id,
                    if *is_error { " error" } else { "" },
                    rendered_content
                ));
            }
            ContentBlock::Thinking { thinking, .. } => {
                if !thinking.trim().is_empty() {
                    rendered.push(format!("[Thinking omitted: {}]", thinking.trim()));
                }
            }
            ContentBlock::RedactedThinking { .. } => {
                rendered.push("[Redacted thinking]".to_string());
            }
            ContentBlock::Image { .. } => {
                rendered.push("[Image omitted]".to_string());
            }
        }
    }
    rendered.join("\n")
}

fn is_profile_noise_start(line: &str) -> bool {
    let trimmed = line.trim();
    (trimmed.starts_with(". :") || trimmed.starts_with(".:")) && trimmed.contains("profile.ps1")
}

fn is_output_encoding_context_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with("\u{6240}\u{5728}\u{4f4d}\u{7f6e}")
        || trimmed.starts_with("At line:")
        || trimmed.starts_with("+ [Console]::OutputEncoding")
        || trimmed.starts_with("+ ~")
        || trimmed.starts_with("+ CategoryInfo")
        || trimmed.starts_with("+ FullyQualifiedErrorId")
        || trimmed.contains("PropertySetterNotSupportedInConstrainedLanguage")
        || trimmed.contains("\u{65e0}\u{6cd5}\u{8bbe}\u{7f6e}")
        || trimmed.contains("Exception setting")
}

fn skip_noise_block<'a>(
    lines: &mut std::iter::Peekable<impl Iterator<Item = &'a str>>,
) {
    while let Some(next) = lines.peek() {
        if next.trim().is_empty() {
            lines.next();
            break;
        }
        lines.next();
    }
}

fn skip_output_encoding_block<'a>(
    lines: &mut std::iter::Peekable<impl Iterator<Item = &'a str>>,
) {
    while let Some(next) = lines.peek() {
        if next.trim().is_empty() {
            lines.next();
            break;
        }
        if is_output_encoding_context_line(next) {
            lines.next();
            continue;
        }
        break;
    }
}

fn find_codex_program() -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    let candidates: &[&str] = if cfg!(windows) {
        &["codex.cmd", "codex.exe", "codex"]
    } else {
        &["codex"]
    };

    for dir in std::env::split_paths(&path_var) {
        for candidate in candidates {
            let full = dir.join(candidate);
            if full.is_file() {
                return Some(full);
            }
        }
    }

    if cfg!(windows) {
        Some(PathBuf::from("codex.cmd"))
    } else {
        Some(PathBuf::from("codex"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::message::{AssistantMessage, UserMessage};
    use uuid::Uuid;

    #[test]
    fn normalizes_backend_name() {
        assert_eq!(normalize_backend(None), NATIVE_BACKEND_NAME);
        assert_eq!(normalize_backend(Some("codex")), CODEX_BACKEND_NAME);
        assert_eq!(normalize_backend(Some("CODEX")), CODEX_BACKEND_NAME);
        assert_eq!(normalize_backend(Some("other")), NATIVE_BACKEND_NAME);
    }

    #[test]
    fn parses_command_execution_event() {
        let raw = serde_json::json!({
            "type": "item.completed",
            "item": {
                "id": "item_1",
                "type": "command_execution",
                "command": "pwd",
                "aggregated_output": "F:/repo",
                "exit_code": 0,
                "status": "completed"
            }
        });
        let parsed: RawCodexEvent = serde_json::from_value(raw).expect("parse raw event");
        let event = CodexExecEvent::try_from(parsed).expect("convert event");

        assert_eq!(
            event,
            CodexExecEvent::ItemCompleted(CodexExecItem::CommandExecution {
                id: "item_1".to_string(),
                command: "pwd".to_string(),
                aggregated_output: "F:/repo".to_string(),
                exit_code: Some(0),
                status: Some("completed".to_string()),
            })
        );
    }

    #[test]
    fn builds_codex_prompt_with_history() {
        let messages = vec![
            Message::User(UserMessage {
                uuid: Uuid::new_v4(),
                timestamp: 1,
                role: "user".to_string(),
                content: MessageContent::Text("Refactor the parser".to_string()),
                is_meta: false,
                tool_use_result: None,
                source_tool_assistant_uuid: None,
            }),
            Message::Assistant(AssistantMessage {
                uuid: Uuid::new_v4(),
                timestamp: 2,
                role: "assistant".to_string(),
                content: vec![
                    ContentBlock::Text {
                        text: "I will inspect the current implementation.".to_string(),
                    },
                    ContentBlock::ToolUse {
                        id: "tool-1".to_string(),
                        name: "Read".to_string(),
                        input: serde_json::json!({"file_path":"src/parser.rs"}),
                    },
                ],
                usage: None,
                stop_reason: None,
                is_api_error_message: false,
                api_error: None,
                cost_usd: 0.0,
            }),
        ];

        let prompt = build_codex_prompt(&messages, Some("Keep changes minimal."), None);
        assert!(prompt.contains("Continue the following cc-rust conversation"));
        assert!(prompt.contains("Additional instructions:\nKeep changes minimal."));
        assert!(prompt.contains("User: Refactor the parser"));
        assert!(prompt.contains("Assistant: I will inspect the current implementation."));
        assert!(prompt.contains("[Tool use: Read"));
    }

    #[test]
    fn sanitizes_powershell_output_encoding_noise() {
        let raw = ".cargo\r\n.claude\r\n无法设置属性。此语言模式仅支持核心类型的属性设置。\r\n所在位置 行:1 字符: 1\r\n+ [Console]::OutputEncoding=[System.Text.Encoding]::UTF8;\r\n+ ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~\r\n    + CategoryInfo          : InvalidOperation: (:) []，RuntimeException\r\n    + FullyQualifiedErrorId : PropertySetterNotSupportedInConstrainedLanguage\r\n";

        assert_eq!(sanitize_command_output(raw), ".cargo\n.claude");
    }
}
