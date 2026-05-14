use cc_types::message::{
    Attachment, ContentBlock, Message, MessageContent, ToolResultContent,
};

pub(in crate::ui) fn message_copy_text(msg: &Message) -> String {
    match msg {
        Message::User(user) => message_content_copy_text(&user.content),
        Message::Assistant(assistant) => assistant
            .content
            .iter()
            .filter_map(content_block_copy_text)
            .collect::<Vec<_>>()
            .join("\n"),
        Message::System(system) => system.content.clone(),
        Message::Progress(progress) => progress
            .data
            .get("message")
            .and_then(|value| value.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| progress.data.to_string()),
        Message::Attachment(attachment) => attachment_copy_text(&attachment.attachment),
    }
}

pub(in crate::ui) fn message_primary_reference(msg: &Message) -> Option<String> {
    match msg {
        Message::User(user) => message_content_reference(&user.content),
        Message::Assistant(assistant) => assistant.content.iter().find_map(content_block_reference),
        Message::Attachment(attachment) => attachment_reference(&attachment.attachment),
        Message::System(_) | Message::Progress(_) => None,
    }
}

pub(in crate::ui) fn message_content_copy_text(content: &MessageContent) -> String {
    match content {
        MessageContent::Text(text) => strip_system_reminders(text),
        MessageContent::Blocks(blocks) => blocks
            .iter()
            .filter_map(content_block_copy_text)
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

pub(in crate::ui) fn content_block_copy_text(block: &ContentBlock) -> Option<String> {
    match block {
        ContentBlock::Text { text } => Some(strip_system_reminders(text)),
        ContentBlock::ConnectorText { connector_text, .. } => Some(connector_text.clone()),
        ContentBlock::ToolUse { name, input, .. }
        | ContentBlock::ServerToolUse { name, input, .. } => {
            tool_primary_input(name, input).or_else(|| Some(input.to_string()))
        }
        ContentBlock::ToolResult { content, .. } => Some(tool_result_content_text(content)),
        ContentBlock::Image { source } => Some(image_reference(source)),
        ContentBlock::Thinking { thinking, .. } => (!thinking.is_empty()).then(|| thinking.clone()),
        ContentBlock::RedactedThinking { .. } => Some("[redacted thinking]".to_string()),
    }
}

pub(in crate::ui) fn image_reference(source: &cc_types::message::ImageSource) -> String {
    format!(
        "[image: {}, {} chars]",
        source.media_type,
        source.data.len()
    )
}

pub(in crate::ui) fn tool_input_summary(
    name: &str,
    input: &serde_json::Value,
    max_chars: usize,
) -> String {
    if let Some(primary) = tool_primary_input(name, input) {
        let json = crate::ui::messages::render::abbreviate_json(input, max_chars);
        return format!("{primary} {json}").trim().to_string();
    }
    crate::ui::messages::render::abbreviate_json(input, max_chars)
}

fn tool_primary_input(name: &str, input: &serde_json::Value) -> Option<String> {
    let key = match name {
        "Read" | "Edit" | "Write" => "file_path",
        "NotebookEdit" => "notebook_path",
        "Bash" => "command",
        "Grep" | "Glob" => "pattern",
        "WebFetch" => "url",
        "WebSearch" => "query",
        "Task" | "Agent" => "prompt",
        _ => return None,
    };
    input
        .get(key)
        .and_then(|value| value.as_str())
        .filter(|value| !value.is_empty())
        .map(|value| {
            let label = if key.ends_with("path") || key == "file_path" {
                "path"
            } else {
                key
            };
            format!("{label}={value}")
        })
}

fn message_content_reference(content: &MessageContent) -> Option<String> {
    match content {
        MessageContent::Text(_) => None,
        MessageContent::Blocks(blocks) => blocks.iter().find_map(content_block_reference),
    }
}

fn content_block_reference(block: &ContentBlock) -> Option<String> {
    match block {
        ContentBlock::ToolUse { name, input, .. }
        | ContentBlock::ServerToolUse { name, input, .. } => tool_primary_input(name, input),
        ContentBlock::Image { source } => Some(image_reference(source)),
        ContentBlock::ToolResult { content, .. } => match content {
            ToolResultContent::Text(_) => None,
            ToolResultContent::Blocks(blocks) => blocks.iter().find_map(content_block_reference),
        },
        _ => None,
    }
}

fn attachment_copy_text(attachment: &Attachment) -> String {
    match attachment {
        Attachment::EditedTextFile { path } => path.clone(),
        Attachment::QueuedCommand { prompt, .. } => prompt.clone(),
        Attachment::MaxTurnsReached {
            max_turns,
            turn_count,
        } => format!("Max turns reached: {turn_count}/{max_turns}"),
        Attachment::StructuredOutput { data } => data.to_string(),
        Attachment::HookStoppedContinuation => "Hook stopped continuation".to_string(),
        Attachment::NestedMemory { path, content } => format!("{path}\n{content}"),
        Attachment::SkillDiscovery { skills } => skills.join(", "),
    }
}

fn attachment_reference(attachment: &Attachment) -> Option<String> {
    match attachment {
        Attachment::EditedTextFile { path } | Attachment::NestedMemory { path, .. } => {
            Some(format!("path={path}"))
        }
        Attachment::QueuedCommand { prompt, .. } => Some(format!(
            "prompt={}",
            cc_utils::messages::truncate_text(prompt, 48)
        )),
        _ => None,
    }
}

fn strip_system_reminders(text: &str) -> String {
    const OPEN: &str = "<system-reminder>";
    const CLOSE: &str = "</system-reminder>";
    let mut rest = text.trim_start();
    while let Some(after_open) = rest.strip_prefix(OPEN) {
        let Some(end) = after_open.find(CLOSE) else {
            break;
        };
        rest = after_open[end + CLOSE.len()..].trim_start();
    }
    rest.to_string()
}

fn tool_result_content_text(content: &ToolResultContent) -> String {
    match content {
        ToolResultContent::Text(text) => text.clone(),
        ToolResultContent::Blocks(blocks) => blocks
            .iter()
            .filter_map(content_block_copy_text)
            .collect::<Vec<_>>()
            .join("\n"),
    }
}
