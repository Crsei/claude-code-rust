use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use std::collections::{HashMap, HashSet};

use crate::ui::markdown::markdown_to_lines;
use crate::ui::messages::assistant_text_message::{classify_assistant_text, render_api_error};
use crate::ui::messages::assistant_thinking_message::{
    render_assistant_thinking_lines, AssistantThinkingView,
};
use crate::ui::messages::assistant_tool_use_message::{
    render_assistant_tool_use_message, ToolUseState,
};
use crate::ui::messages::attachment_message::render_attachment_message as render_attachment_helper;
use crate::ui::messages::collapsed_read_search_content::{
    render_collapsed_read_search_lines, CollapsedReadSearchView,
};
use crate::ui::messages::compact_boundary_message::render_compact_boundary_lines;
use crate::ui::messages::grouped_tool_use_content::{
    render_grouped_tool_use_lines, GroupedToolUseView,
};
use crate::ui::messages::system_text_message::render_system_text_message;
use crate::ui::messages::user_bash_output_message::{
    render_user_bash_output_message_with_options, ShellOutputRenderOptions,
};
use crate::ui::messages::user_text_message::render_user_text_message;
use crate::ui::messages::user_tool_result_message::user_tool_result_message::render_user_tool_result_message;
use crate::ui::messages::user_tool_result_message::utils::{
    ToolResultBlock, ToolUseRecord as UserToolUseRecord, UserToolResultLookups,
};
use crate::ui::theme::Theme;
use crate::ui::virtual_scroll::VirtualScroll;
use cc_types::message::{
    Attachment, ContentBlock, InfoLevel, Message, MessageContent, SystemSubtype, ToolResultContent,
};

use super::file_edit_tool_updated_message::{
    render_file_edit_tool_updated_message, FileEditMessageStyle, FileEditToolUpdatedView,
};
use super::wrap::wrap_line_to_width;

#[derive(Debug, Clone, Default)]
pub(crate) struct MessageRenderContext {
    renderable_messages: Vec<RenderableMessage>,
    lookups: MessageLookups,
    selected_message: Option<usize>,
    selected_expanded: bool,
    options: MessageRenderOptions,
    cache_key: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct MessageRenderOptions {
    pub(crate) verbose: bool,
    pub(crate) is_transcript_mode: bool,
    pub(crate) show_all_in_transcript: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct PreparedMessages {
    pub(crate) renderable: Vec<RenderableMessage>,
    pub(crate) lookups: MessageLookups,
}

#[derive(Debug, Clone)]
pub(crate) enum RenderableMessage {
    Message {
        message: Message,
        source_index: usize,
    },
    GroupedToolUse(GroupedToolUseRenderRecord),
    CollapsedReadSearch(CollapsedReadSearchRenderRecord),
}

#[derive(Debug, Clone)]
pub(crate) struct GroupedToolUseRenderRecord {
    uuid: uuid::Uuid,
    timestamp: i64,
    source_indices: Vec<usize>,
    tool_name: String,
    tool_use_ids: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct CollapsedReadSearchRenderRecord {
    uuid: uuid::Uuid,
    timestamp: i64,
    source_indices: Vec<usize>,
    tool_use_ids: Vec<String>,
    read_count: usize,
    search_count: usize,
    list_count: usize,
    latest_hint: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct MessageLookups {
    tool_uses: HashMap<String, ToolUseRenderRecord>,
    tool_results: HashMap<String, ToolResultRenderRecord>,
    resolved_tool_use_ids: HashSet<String>,
    errored_tool_use_ids: HashSet<String>,
    in_progress_tool_use_ids: HashSet<String>,
    progress_messages_by_tool_use_id: HashMap<String, Vec<cc_types::message::ProgressMessage>>,
    latest_shell_tool_result_id: Option<String>,
    latest_bash_output_uuid: Option<uuid::Uuid>,
    last_thinking_block_id: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct ToolUseRenderRecord {
    tool_name: String,
    input: serde_json::Value,
}

#[derive(Debug, Clone)]
pub(crate) struct ToolResultRenderRecord {
    tool_use_id: String,
    content: String,
    is_error: bool,
    tool_use_result: Option<String>,
    message_uuid: uuid::Uuid,
    source_index: usize,
}

impl MessageRenderContext {
    pub(crate) fn cache_key(&self) -> &str {
        &self.cache_key
    }

    pub(crate) fn renderable_messages(&self) -> &[RenderableMessage] {
        &self.renderable_messages
    }

    fn tool_use(&self, tool_use_id: &str) -> Option<&ToolUseRenderRecord> {
        self.lookups.tool_uses.get(tool_use_id)
    }

    fn shell_expanded(&self, msg_index: usize, tool_use_id: &str) -> bool {
        self.lookups.latest_shell_tool_result_id.as_deref() == Some(tool_use_id)
            || (self.selected_message == Some(msg_index) && self.selected_expanded)
    }
}

#[cfg(test)]
pub(crate) fn build_message_render_context(
    messages: &[Message],
    selected_message: Option<usize>,
    selected_expanded: bool,
) -> MessageRenderContext {
    build_message_render_context_with_options(
        messages,
        selected_message,
        selected_expanded,
        MessageRenderOptions::default(),
    )
}

pub(crate) fn build_message_render_context_with_options(
    messages: &[Message],
    selected_message: Option<usize>,
    selected_expanded: bool,
    options: MessageRenderOptions,
) -> MessageRenderContext {
    let prepared = prepare_renderable_messages(messages, options);
    let mut ctx = MessageRenderContext {
        renderable_messages: prepared.renderable,
        lookups: prepared.lookups,
        selected_message,
        selected_expanded,
        options,
        ..MessageRenderContext::default()
    };

    ctx.cache_key = render_context_cache_key(&ctx);
    ctx
}

pub(crate) fn prepare_renderable_messages(
    messages: &[Message],
    options: MessageRenderOptions,
) -> PreparedMessages {
    let normalized = normalize_messages_for_render(messages)
        .into_iter()
        .filter(is_not_empty_renderable_message)
        .collect::<Vec<_>>();
    let compact_aware = filter_compact_boundary(normalized.clone(), options);
    let visible = compact_aware
        .into_iter()
        .filter(|msg| should_show_renderable_message(msg, options))
        .collect::<Vec<_>>();
    let reordered = reorder_messages_in_ui(visible);
    let brief_filtered = filter_brief_messages(reordered, options);
    let transcript_limited = truncate_transcript_messages(brief_filtered, options);
    let grouped = apply_grouping(transcript_limited, options);
    let collapsed = collapse_read_search_groups(grouped, options);
    let lookups = build_message_lookups(&normalized, &collapsed);

    PreparedMessages {
        renderable: collapsed,
        lookups,
    }
}

pub(crate) fn build_message_lookups(
    normalized: &[RenderableMessage],
    _renderable: &[RenderableMessage],
) -> MessageLookups {
    let mut lookups = MessageLookups {
        latest_bash_output_uuid: find_latest_bash_output_uuid(normalized),
        last_thinking_block_id: find_last_thinking_block_id(normalized),
        ..MessageLookups::default()
    };

    for render_msg in normalized {
        for (message, source_index) in render_msg.messages_for_lookup() {
            match message {
                Message::Assistant(assistant) => {
                    for block in &assistant.content {
                        match block {
                            ContentBlock::ToolUse { id, name, input }
                            | ContentBlock::ServerToolUse { id, name, input } => {
                                lookups.tool_uses.insert(
                                    id.clone(),
                                    ToolUseRenderRecord {
                                        tool_name: name.clone(),
                                        input: input.clone(),
                                    },
                                );
                            }
                            _ => {}
                        }
                    }
                }
                Message::User(user) => {
                    for block in message_content_blocks(&user.content) {
                        if let ContentBlock::ToolResult {
                            tool_use_id,
                            content,
                            is_error,
                        } = block
                        {
                            let record = ToolResultRenderRecord {
                                tool_use_id: tool_use_id.clone(),
                                content: tool_result_content_text(content),
                                is_error: *is_error,
                                tool_use_result: user.tool_use_result.clone(),
                                message_uuid: user.uuid,
                                source_index,
                            };
                            if *is_error {
                                lookups.errored_tool_use_ids.insert(tool_use_id.clone());
                            }
                            lookups.resolved_tool_use_ids.insert(tool_use_id.clone());
                            lookups.tool_results.insert(tool_use_id.clone(), record);
                        }
                    }
                }
                Message::Progress(progress) => {
                    lookups
                        .progress_messages_by_tool_use_id
                        .entry(progress.tool_use_id.clone())
                        .or_default()
                        .push(progress.clone());
                }
                _ => {}
            }
        }
    }

    for (id, tool_use) in &lookups.tool_uses {
        if !lookups.resolved_tool_use_ids.contains(id) {
            lookups.in_progress_tool_use_ids.insert(id.clone());
        }
        if tool_use.is_shell() && lookups.resolved_tool_use_ids.contains(id) {
            lookups.latest_shell_tool_result_id = Some(id.clone());
        }
    }

    lookups
}

fn render_context_cache_key(ctx: &MessageRenderContext) -> String {
    let mut parts = vec![
        format!(
            "mode=v{}t{}",
            u8::from(ctx.options.verbose),
            u8::from(ctx.options.is_transcript_mode)
        ),
        format!(
            "shell={}",
            ctx.lookups
                .latest_shell_tool_result_id
                .as_deref()
                .unwrap_or("")
        ),
        format!("selected={:?}", ctx.selected_message),
        format!("expanded={}", ctx.selected_expanded),
        format!(
            "thinking={}",
            ctx.lookups.last_thinking_block_id.as_deref().unwrap_or("")
        ),
    ];
    parts.extend(ctx.lookups.tool_results.values().map(|result| {
        format!(
            "r:{}:{}:{}:{}:{}:{}",
            result.tool_use_id,
            result.content.len(),
            result.is_error,
            result.tool_use_result.as_deref().unwrap_or("").len(),
            result.message_uuid,
            result.source_index
        )
    }));
    parts.extend(
        ctx.renderable_messages
            .iter()
            .map(RenderableMessage::cache_part),
    );
    parts.join("|")
}

impl RenderableMessage {
    fn uuid(&self) -> uuid::Uuid {
        match self {
            RenderableMessage::Message { message, .. } => message.uuid(),
            RenderableMessage::GroupedToolUse(group) => group.uuid,
            RenderableMessage::CollapsedReadSearch(group) => group.uuid,
        }
    }

    fn timestamp(&self) -> i64 {
        match self {
            RenderableMessage::Message { message, .. } => message.timestamp(),
            RenderableMessage::GroupedToolUse(group) => group.timestamp,
            RenderableMessage::CollapsedReadSearch(group) => group.timestamp,
        }
    }

    fn cache_part(&self) -> String {
        match self {
            RenderableMessage::Message { message, .. } => {
                format!("m:{}:{}", message_type_key(message), message.uuid())
            }
            RenderableMessage::GroupedToolUse(group) => {
                format!("g:{}:{}", group.tool_name, group.uuid)
            }
            RenderableMessage::CollapsedReadSearch(group) => {
                format!(
                    "c:{}:{}:{}:{}",
                    group.read_count, group.search_count, group.list_count, group.uuid
                )
            }
        }
    }

    fn is_assistant_message(&self) -> bool {
        matches!(
            self,
            RenderableMessage::Message {
                message: Message::Assistant(_),
                ..
            }
        )
    }

    fn has_source_index(&self, selected: Option<usize>) -> bool {
        let Some(selected) = selected else {
            return false;
        };
        match self {
            RenderableMessage::Message { source_index, .. } => *source_index == selected,
            RenderableMessage::GroupedToolUse(group) => group.source_indices.contains(&selected),
            RenderableMessage::CollapsedReadSearch(group) => {
                group.source_indices.contains(&selected)
            }
        }
    }

    fn messages_for_lookup(&self) -> Vec<(&Message, usize)> {
        match self {
            RenderableMessage::Message {
                message,
                source_index,
            } => vec![(message, *source_index)],
            RenderableMessage::GroupedToolUse(_) | RenderableMessage::CollapsedReadSearch(_) => {
                Vec::new()
            }
        }
    }
}

fn normalize_messages_for_render(messages: &[Message]) -> Vec<RenderableMessage> {
    let mut normalized = Vec::new();
    let mut split_seen = false;

    for (source_index, message) in messages.iter().enumerate() {
        match message {
            Message::Assistant(assistant) => {
                split_seen |= assistant.content.len() > 1;
                for (block_index, block) in assistant.content.iter().cloned().enumerate() {
                    let mut msg = assistant.clone();
                    msg.uuid = if split_seen {
                        derive_child_uuid(assistant.uuid, block_index)
                    } else {
                        assistant.uuid
                    };
                    msg.content = vec![block];
                    normalized.push(RenderableMessage::Message {
                        message: Message::Assistant(msg),
                        source_index,
                    });
                }
            }
            Message::User(user) => {
                let blocks = match &user.content {
                    MessageContent::Text(text) => vec![ContentBlock::Text { text: text.clone() }],
                    MessageContent::Blocks(blocks) => blocks.clone(),
                };
                split_seen |= blocks.len() > 1;
                for (block_index, block) in blocks.into_iter().enumerate() {
                    let mut msg = user.clone();
                    msg.uuid = if split_seen {
                        derive_child_uuid(user.uuid, block_index)
                    } else {
                        user.uuid
                    };
                    msg.content = MessageContent::Blocks(vec![block]);
                    normalized.push(RenderableMessage::Message {
                        message: Message::User(msg),
                        source_index,
                    });
                }
            }
            Message::System(_) | Message::Progress(_) | Message::Attachment(_) => {
                normalized.push(RenderableMessage::Message {
                    message: message.clone(),
                    source_index,
                });
            }
        }
    }

    normalized
}

fn derive_child_uuid(parent: uuid::Uuid, index: usize) -> uuid::Uuid {
    let parent = parent.to_string();
    let derived = format!("{}{:012x}", &parent[..24], index);
    uuid::Uuid::parse_str(&derived)
        .unwrap_or_else(|_| uuid_from_parent_and_salt(uuid::Uuid::nil(), &derived))
}

fn is_not_empty_renderable_message(msg: &RenderableMessage) -> bool {
    let RenderableMessage::Message { message, .. } = msg else {
        return true;
    };
    match message {
        Message::Progress(_) | Message::Attachment(_) | Message::System(_) => true,
        Message::Assistant(assistant) => {
            if assistant.content.is_empty() {
                return false;
            }
            if assistant.content.len() > 1 {
                return true;
            }
            match &assistant.content[0] {
                ContentBlock::Text { text } => !is_empty_message_text(text),
                _ => true,
            }
        }
        Message::User(user) => match &user.content {
            MessageContent::Text(text) => !is_empty_message_text(text),
            MessageContent::Blocks(blocks) => {
                if blocks.is_empty() {
                    return false;
                }
                if blocks.len() > 1 {
                    return true;
                }
                match &blocks[0] {
                    ContentBlock::Text { text } => {
                        !is_empty_message_text(text)
                            && text.trim()
                                != crate::ui::messages::user_tool_result_message::utils::INTERRUPT_MESSAGE_FOR_TOOL_USE
                    }
                    _ => true,
                }
            }
        },
    }
}

fn is_empty_message_text(text: &str) -> bool {
    let stripped = strip_prompt_xml_tags(text);
    stripped.trim().is_empty() || stripped.trim() == "[NO_CONTENT]"
}

fn strip_prompt_xml_tags(text: &str) -> String {
    let mut out = text.to_string();
    for tag in [
        "commit_analysis",
        "context",
        "function_analysis",
        "pr_analysis",
    ] {
        out = strip_simple_tag(&out, tag);
    }
    out.trim().to_string()
}

fn strip_simple_tag(text: &str, tag: &str) -> String {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let mut rest = text;
    let mut out = String::new();
    while let Some(start) = rest.find(&open) {
        out.push_str(&rest[..start]);
        let after_open = &rest[start + open.len()..];
        let Some(end) = after_open.find(&close) else {
            out.push_str(after_open);
            return out;
        };
        rest = &after_open[end + close.len()..];
    }
    out.push_str(rest);
    out
}

fn filter_compact_boundary(
    messages: Vec<RenderableMessage>,
    options: MessageRenderOptions,
) -> Vec<RenderableMessage> {
    if options.verbose || options.is_transcript_mode {
        return messages;
    }
    let boundary_index = messages.iter().rposition(|msg| {
        matches!(
            msg,
            RenderableMessage::Message {
                message: Message::System(cc_types::message::SystemMessage {
                    subtype: SystemSubtype::CompactBoundary { .. },
                    ..
                }),
                ..
            }
        )
    });
    boundary_index
        .map(|idx| messages[idx..].to_vec())
        .unwrap_or(messages)
}

fn should_show_renderable_message(msg: &RenderableMessage, options: MessageRenderOptions) -> bool {
    let RenderableMessage::Message { message, .. } = msg else {
        return true;
    };
    match message {
        Message::Progress(_) => false,
        Message::Attachment(attachment) => !is_null_rendering_attachment(&attachment.attachment),
        Message::User(user) => {
            if user.is_meta && !options.is_transcript_mode && !user_contains_tool_result(user) {
                return false;
            }
            true
        }
        Message::Assistant(_) | Message::System(_) => true,
    }
}

fn is_null_rendering_attachment(attachment: &Attachment) -> bool {
    matches!(
        attachment,
        Attachment::EditedTextFile { .. }
            | Attachment::MaxTurnsReached { .. }
            | Attachment::StructuredOutput { .. }
    )
}

fn user_contains_tool_result(user: &cc_types::message::UserMessage) -> bool {
    message_content_blocks(&user.content)
        .iter()
        .any(|block| matches!(block, ContentBlock::ToolResult { .. }))
}

fn reorder_messages_in_ui(messages: Vec<RenderableMessage>) -> Vec<RenderableMessage> {
    let mut tool_results = HashMap::<String, RenderableMessage>::new();
    for msg in &messages {
        if let Some(tool_use_id) = tool_result_id(msg) {
            tool_results.insert(tool_use_id.to_string(), msg.clone());
        }
    }

    let mut result = Vec::new();
    let mut consumed_results = HashSet::<String>::new();
    for msg in messages {
        if let Some(tool_use_id) = tool_use_id(&msg).map(str::to_string) {
            result.push(msg);
            if let Some(result_msg) = tool_results.get(&tool_use_id) {
                result.push(result_msg.clone());
                consumed_results.insert(tool_use_id);
            }
            continue;
        }
        if let Some(tool_use_id) = tool_result_id(&msg) {
            if consumed_results.contains(tool_use_id) {
                continue;
            }
        }
        if is_api_error_message(&msg) {
            if result.last().is_some_and(is_api_error_message) {
                result.pop();
            }
            result.push(msg);
            continue;
        }
        result.push(msg);
    }

    let last_idx = result.len().saturating_sub(1);
    result
        .into_iter()
        .enumerate()
        .filter_map(|(idx, msg)| {
            if is_api_error_message(&msg) && idx != last_idx {
                None
            } else {
                Some(msg)
            }
        })
        .collect()
}

fn filter_brief_messages(
    messages: Vec<RenderableMessage>,
    _options: MessageRenderOptions,
) -> Vec<RenderableMessage> {
    messages
}

fn truncate_transcript_messages(
    messages: Vec<RenderableMessage>,
    options: MessageRenderOptions,
) -> Vec<RenderableMessage> {
    const MAX_MESSAGES_TO_SHOW_IN_TRANSCRIPT_MODE: usize = 30;
    if options.is_transcript_mode
        && !options.show_all_in_transcript
        && messages.len() > MAX_MESSAGES_TO_SHOW_IN_TRANSCRIPT_MODE
    {
        messages[messages.len() - MAX_MESSAGES_TO_SHOW_IN_TRANSCRIPT_MODE..].to_vec()
    } else {
        messages
    }
}

fn apply_grouping(
    messages: Vec<RenderableMessage>,
    options: MessageRenderOptions,
) -> Vec<RenderableMessage> {
    if options.verbose {
        return messages;
    }

    let mut groups: HashMap<(usize, String), Vec<RenderableMessage>> = HashMap::new();
    for msg in &messages {
        if let Some((source_index, name)) = grouping_tool_use_key(msg) {
            groups
                .entry((source_index, name.to_string()))
                .or_default()
                .push(msg.clone());
        }
    }
    let valid = groups
        .into_iter()
        .filter(|(_, group)| group.len() >= 2)
        .collect::<HashMap<_, _>>();

    if valid.is_empty() {
        return messages;
    }

    let mut emitted = HashSet::<(usize, String)>::new();
    let mut grouped_tool_ids = HashSet::<String>::new();
    for group in valid.values() {
        for msg in group {
            if let Some(id) = tool_use_id(msg) {
                grouped_tool_ids.insert(id.to_string());
            }
        }
    }

    let mut result = Vec::new();
    for msg in messages {
        if let Some((source_index, name)) = grouping_tool_use_key(&msg) {
            let key = (source_index, name.to_string());
            if let Some(group) = valid.get(&key) {
                if emitted.insert(key.clone()) {
                    let tool_use_ids = group
                        .iter()
                        .filter_map(|msg| tool_use_id(msg).map(str::to_string))
                        .collect::<Vec<_>>();
                    result.push(RenderableMessage::GroupedToolUse(
                        GroupedToolUseRenderRecord {
                            uuid: derive_group_uuid(group[0].uuid(), "grouped"),
                            timestamp: group[0].timestamp(),
                            source_indices: group
                                .iter()
                                .filter_map(source_index_of)
                                .collect::<HashSet<_>>()
                                .into_iter()
                                .collect(),
                            tool_name: name.to_string(),
                            tool_use_ids,
                        },
                    ));
                }
                continue;
            }
        }
        if let Some(tool_use_id) = tool_result_id(&msg) {
            if grouped_tool_ids.contains(tool_use_id) {
                continue;
            }
        }
        result.push(msg);
    }

    result
}

fn collapse_read_search_groups(
    messages: Vec<RenderableMessage>,
    options: MessageRenderOptions,
) -> Vec<RenderableMessage> {
    if options.verbose {
        return messages;
    }

    let mut result = Vec::new();
    let mut i = 0usize;
    while i < messages.len() {
        let Some(first_info) = collapsible_tool_info(&messages[i]) else {
            result.push(messages[i].clone());
            i += 1;
            continue;
        };

        let mut originals = vec![messages[i].clone()];
        let mut source_indices = HashSet::new();
        let mut tool_use_ids = Vec::new();
        let mut read_count = 0usize;
        let mut search_count = 0usize;
        let mut list_count = 0usize;
        let mut latest_hint = None;
        add_collapsible_info(
            first_info,
            &messages[i],
            &mut source_indices,
            &mut tool_use_ids,
            &mut read_count,
            &mut search_count,
            &mut list_count,
            &mut latest_hint,
        );
        i += 1;

        while i < messages.len() {
            if let Some(result_id) = tool_result_id(&messages[i]) {
                if tool_use_ids.iter().any(|id| id == result_id) {
                    originals.push(messages[i].clone());
                    if let Some(source_index) = source_index_of(&messages[i]) {
                        source_indices.insert(source_index);
                    }
                    i += 1;
                    continue;
                }
            }
            let Some(info) = collapsible_tool_info(&messages[i]) else {
                break;
            };
            add_collapsible_info(
                info,
                &messages[i],
                &mut source_indices,
                &mut tool_use_ids,
                &mut read_count,
                &mut search_count,
                &mut list_count,
                &mut latest_hint,
            );
            originals.push(messages[i].clone());
            i += 1;
        }

        let tool_count = read_count + search_count + list_count;
        if tool_count >= 2 {
            result.push(RenderableMessage::CollapsedReadSearch(
                CollapsedReadSearchRenderRecord {
                    uuid: derive_group_uuid(originals[0].uuid(), "collapsed"),
                    timestamp: originals[0].timestamp(),
                    source_indices: source_indices.into_iter().collect(),
                    tool_use_ids,
                    read_count,
                    search_count,
                    list_count,
                    latest_hint,
                },
            ));
        } else {
            result.extend(originals);
        }
    }
    result
}

#[derive(Debug, Clone)]
struct CollapsibleToolInfo {
    tool_use_ids: Vec<String>,
    read_count: usize,
    search_count: usize,
    list_count: usize,
    hint: Option<String>,
}

fn add_collapsible_info(
    info: CollapsibleToolInfo,
    msg: &RenderableMessage,
    source_indices: &mut HashSet<usize>,
    tool_use_ids: &mut Vec<String>,
    read_count: &mut usize,
    search_count: &mut usize,
    list_count: &mut usize,
    latest_hint: &mut Option<String>,
) {
    if let Some(source_index) = source_index_of(msg) {
        source_indices.insert(source_index);
    }
    tool_use_ids.extend(info.tool_use_ids);
    *read_count += info.read_count;
    *search_count += info.search_count;
    *list_count += info.list_count;
    if info.hint.is_some() {
        *latest_hint = info.hint;
    }
}

fn collapsible_tool_info(msg: &RenderableMessage) -> Option<CollapsibleToolInfo> {
    match msg {
        RenderableMessage::Message {
            message: Message::Assistant(assistant),
            ..
        } => {
            let block = assistant.content.first()?;
            let (ContentBlock::ToolUse { id, name, input }
            | ContentBlock::ServerToolUse { id, name, input }) = block
            else {
                return None;
            };
            collapsible_info_for_tool(id, name, input)
        }
        RenderableMessage::GroupedToolUse(group) => {
            let kind = collapsible_kind_for_tool(&group.tool_name)?;
            let (read_count, search_count, list_count) =
                counts_for_kind(kind, group.tool_use_ids.len());
            Some(CollapsibleToolInfo {
                tool_use_ids: group.tool_use_ids.clone(),
                read_count,
                search_count,
                list_count,
                hint: Some(group.tool_name.clone()),
            })
        }
        _ => None,
    }
}

fn collapsible_info_for_tool(
    id: &str,
    name: &str,
    input: &serde_json::Value,
) -> Option<CollapsibleToolInfo> {
    let kind = collapsible_kind_for_tool(name)?;
    let (read_count, search_count, list_count) = counts_for_kind(kind, 1);
    Some(CollapsibleToolInfo {
        tool_use_ids: vec![id.to_string()],
        read_count,
        search_count,
        list_count,
        hint: tool_primary_input(name, input),
    })
}

#[derive(Debug, Clone, Copy)]
enum CollapsibleKind {
    Read,
    Search,
    List,
}

fn collapsible_kind_for_tool(name: &str) -> Option<CollapsibleKind> {
    match name {
        "Read" => Some(CollapsibleKind::Read),
        "Grep" | "Glob" | "WebSearch" => Some(CollapsibleKind::Search),
        "LS" | "List" => Some(CollapsibleKind::List),
        _ => None,
    }
}

fn counts_for_kind(kind: CollapsibleKind, count: usize) -> (usize, usize, usize) {
    match kind {
        CollapsibleKind::Read => (count, 0, 0),
        CollapsibleKind::Search => (0, count, 0),
        CollapsibleKind::List => (0, 0, count),
    }
}

fn grouping_tool_use_key(msg: &RenderableMessage) -> Option<(usize, &str)> {
    let RenderableMessage::Message {
        message: Message::Assistant(assistant),
        source_index,
    } = msg
    else {
        return None;
    };
    let (ContentBlock::ToolUse { name, .. } | ContentBlock::ServerToolUse { name, .. }) =
        assistant.content.first()?
    else {
        return None;
    };
    if is_groupable_tool(name) {
        Some((*source_index, name.as_str()))
    } else {
        None
    }
}

fn is_groupable_tool(name: &str) -> bool {
    matches!(
        name,
        "Task" | "Agent" | "TodoWrite" | "Read" | "Grep" | "Glob"
    )
}

fn source_index_of(msg: &RenderableMessage) -> Option<usize> {
    match msg {
        RenderableMessage::Message { source_index, .. } => Some(*source_index),
        RenderableMessage::GroupedToolUse(group) => group.source_indices.first().copied(),
        RenderableMessage::CollapsedReadSearch(group) => group.source_indices.first().copied(),
    }
}

fn tool_use_id(msg: &RenderableMessage) -> Option<&str> {
    let RenderableMessage::Message {
        message: Message::Assistant(assistant),
        ..
    } = msg
    else {
        return None;
    };
    match assistant.content.first()? {
        ContentBlock::ToolUse { id, .. } | ContentBlock::ServerToolUse { id, .. } => Some(id),
        _ => None,
    }
}

fn tool_result_id(msg: &RenderableMessage) -> Option<&str> {
    let RenderableMessage::Message {
        message: Message::User(user),
        ..
    } = msg
    else {
        return None;
    };
    message_content_blocks(&user.content)
        .iter()
        .find_map(|block| match block {
            ContentBlock::ToolResult { tool_use_id, .. } => Some(tool_use_id.as_str()),
            _ => None,
        })
}

fn is_api_error_message(msg: &RenderableMessage) -> bool {
    matches!(
        msg,
        RenderableMessage::Message {
            message: Message::System(cc_types::message::SystemMessage {
                subtype: SystemSubtype::ApiError { .. },
                ..
            }),
            ..
        }
    )
}

fn find_latest_bash_output_uuid(messages: &[RenderableMessage]) -> Option<uuid::Uuid> {
    messages.iter().rev().find_map(|msg| {
        let RenderableMessage::Message {
            message: Message::User(user),
            ..
        } = msg
        else {
            return None;
        };
        message_content_blocks(&user.content)
            .iter()
            .find_map(|block| {
                let ContentBlock::Text { text } = block else {
                    return None;
                };
                (text.starts_with("<bash-stdout") || text.starts_with("<bash-stderr"))
                    .then_some(user.uuid)
            })
    })
}

fn find_last_thinking_block_id(messages: &[RenderableMessage]) -> Option<String> {
    messages.iter().rev().find_map(|msg| {
        let RenderableMessage::Message {
            message: Message::Assistant(assistant),
            ..
        } = msg
        else {
            return None;
        };
        assistant
            .content
            .iter()
            .enumerate()
            .rev()
            .find_map(|(idx, block)| {
                matches!(
                    block,
                    ContentBlock::Thinking { .. } | ContentBlock::RedactedThinking { .. }
                )
                .then(|| format!("{}:{idx}", assistant.uuid))
            })
    })
}

fn message_content_blocks(content: &MessageContent) -> Vec<&ContentBlock> {
    match content {
        MessageContent::Text(_) => Vec::new(),
        MessageContent::Blocks(blocks) => blocks.iter().collect(),
    }
}

fn derive_group_uuid(parent: uuid::Uuid, salt: &str) -> uuid::Uuid {
    uuid_from_parent_and_salt(parent, salt)
}

fn uuid_from_parent_and_salt(parent: uuid::Uuid, salt: &str) -> uuid::Uuid {
    let mut bytes = *parent.as_bytes();
    for (idx, byte) in salt.as_bytes().iter().enumerate() {
        bytes[idx % bytes.len()] ^= *byte;
    }
    uuid::Uuid::from_bytes(bytes)
}

fn message_type_key(message: &Message) -> &'static str {
    match message {
        Message::User(_) => "u",
        Message::Assistant(_) => "a",
        Message::System(_) => "s",
        Message::Progress(_) => "p",
        Message::Attachment(_) => "t",
    }
}

impl ToolUseRenderRecord {
    fn is_shell(&self) -> bool {
        matches!(self.tool_name.as_str(), "Bash" | "PowerShell")
    }

    fn command(&self) -> String {
        self.input
            .get("command")
            .and_then(|value| value.as_str())
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| self.tool_name.clone())
    }
}

/// Render only the visible messages into the given buffer area using virtual
/// scrolling.
///
/// `vscroll` must have been updated via `ensure_up_to_date()` before calling.
/// `scroll` is the number of rendered lines to skip from the top.
#[expect(
    clippy::too_many_arguments,
    reason = "message renderer takes explicit ratatui render state to avoid per-frame allocations"
)]
pub fn render_messages(
    _messages: &[Message],
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
    streaming: bool,
    scroll: usize,
    vscroll: &VirtualScroll,
    render_context: &MessageRenderContext,
) {
    let renderable_messages = render_context.renderable_messages();
    if area.height == 0 || area.width == 0 || renderable_messages.is_empty() {
        return;
    }

    let viewport_h = area.height as usize;
    let (start, end) = vscroll.visual_range(scroll, viewport_h);

    // Where the first visible message starts in wrapped visual line space.
    let first_offset = vscroll.visual_offset_of(start);
    // How many lines to skip inside the first visible message.
    let skip_in_first = scroll.saturating_sub(first_offset);

    let mut y = 0usize; // current row in the viewport

    for idx in start..end.min(renderable_messages.len()) {
        let mut msg_lines = render_renderable_message_wrapped(
            &renderable_messages[idx],
            idx,
            theme,
            area.width,
            render_context,
        );
        if streaming
            && idx == renderable_messages.len().saturating_sub(1)
            && renderable_messages[idx].is_assistant_message()
        {
            if let Some(last_line) = msg_lines.last_mut() {
                last_line.spans.push(Span::styled(" ▌", theme.dim));
            }
        }

        // Separator blank line (between messages, not after last)
        let has_sep = idx < renderable_messages.len() - 1;
        let total_for_msg = msg_lines.len() + if has_sep { 1 } else { 0 };

        let skip = if idx == start { skip_in_first } else { 0 };

        for li in skip..total_for_msg {
            if y >= viewport_h {
                return;
            }
            let line = if li < msg_lines.len() {
                &msg_lines[li]
            } else {
                // separator blank line
                &Line::default()
            };
            buf.set_line(area.x, area.y + y as u16, line, area.width);
            y += 1;
        }
    }
}

pub(crate) fn render_renderable_message_wrapped<'a>(
    msg: &RenderableMessage,
    index: usize,
    theme: &Theme,
    width: u16,
    render_context: &MessageRenderContext,
) -> Vec<Line<'a>> {
    render_renderable_message_for_layout(msg, index, theme, width as usize, render_context)
        .into_iter()
        .flat_map(|line| wrap_line_to_width(&line, width))
        .collect()
}

pub(crate) fn render_renderable_message_for_layout<'a>(
    msg: &RenderableMessage,
    index: usize,
    theme: &Theme,
    width: usize,
    render_context: &MessageRenderContext,
) -> Vec<Line<'a>> {
    let mut lines =
        render_renderable_message_with_context(msg, index, theme, width, render_context);
    if msg.has_source_index(render_context.selected_message) {
        if let RenderableMessage::Message { message, .. } = msg {
            decorate_selected_message(
                &mut lines,
                message,
                theme,
                render_context.selected_expanded,
                width,
            );
        }
    }
    lines
}

/// Render a single message into one or more `Line`s.
///
/// `pub(super)` so that `virtual_scroll` can call it for height measurement.
#[cfg(test)]
pub(in crate::ui) fn render_single_message<'a>(msg: &Message, theme: &Theme) -> Vec<Line<'a>> {
    render_single_message_with_context(msg, 0, theme, 80, &MessageRenderContext::default())
}

pub(in crate::ui) fn render_single_message_with_context<'a>(
    msg: &Message,
    index: usize,
    theme: &Theme,
    width: usize,
    render_context: &MessageRenderContext,
) -> Vec<Line<'a>> {
    match msg {
        Message::User(user_msg) => {
            render_user_message(user_msg, theme, index, width, render_context)
        }
        Message::Assistant(assistant_msg) => {
            render_assistant_message(assistant_msg, theme, render_context)
        }
        Message::System(system_msg) => render_system_message(system_msg, theme),
        Message::Progress(progress_msg) => render_progress_message(progress_msg, theme),
        Message::Attachment(attachment_msg) => render_attachment_message(attachment_msg, theme),
    }
}

pub(in crate::ui) fn render_renderable_message_with_context<'a>(
    msg: &RenderableMessage,
    index: usize,
    theme: &Theme,
    width: usize,
    render_context: &MessageRenderContext,
) -> Vec<Line<'a>> {
    match msg {
        RenderableMessage::Message { message, .. } => {
            render_single_message_with_context(message, index, theme, width, render_context)
        }
        RenderableMessage::GroupedToolUse(group) => {
            let resolved_count = group
                .tool_use_ids
                .iter()
                .filter(|id| render_context.lookups.resolved_tool_use_ids.contains(*id))
                .count();
            let error_count = group
                .tool_use_ids
                .iter()
                .filter(|id| render_context.lookups.errored_tool_use_ids.contains(*id))
                .count();
            render_grouped_tool_use_lines(
                &GroupedToolUseView {
                    tool_name: group.tool_name.clone(),
                    count: group.tool_use_ids.len(),
                    resolved_count,
                    error_count,
                },
                theme,
            )
        }
        RenderableMessage::CollapsedReadSearch(group) => {
            let active = group
                .tool_use_ids
                .iter()
                .any(|id| render_context.lookups.in_progress_tool_use_ids.contains(id));
            render_collapsed_read_search_lines(
                &CollapsedReadSearchView {
                    read_count: group.read_count,
                    search_count: group.search_count,
                    list_count: group.list_count,
                    active,
                    latest_hint: group.latest_hint.clone(),
                },
                theme,
            )
        }
    }
}

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

fn decorate_selected_message<'a>(
    lines: &mut Vec<Line<'a>>,
    msg: &Message,
    theme: &Theme,
    expanded: bool,
    width: usize,
) {
    let mut header = vec![
        Span::styled(
            if expanded {
                "▼ selected"
            } else {
                "▶ selected"
            },
            theme.selected,
        ),
        Span::styled(" · c copy", theme.dim),
        Span::styled(" · enter detail", theme.dim),
    ];
    if let Some(meta) = selected_message_meta(msg) {
        header.push(Span::styled(format!(" · {meta}"), theme.dim));
    }
    lines.insert(0, Line::from(header));

    if expanded {
        lines.extend(
            message_detail_lines(msg, width)
                .into_iter()
                .map(|line| Line::from(Span::styled(format!("  {line}"), theme.dim))),
        );
    }
}

fn selected_message_meta(msg: &Message) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(ts) = concise_timestamp(msg.timestamp()) {
        parts.push(ts);
    }
    if let Some(reference) = message_primary_reference(msg) {
        parts.push(reference);
    }
    (!parts.is_empty()).then(|| parts.join(" · "))
}

fn concise_timestamp(timestamp: i64) -> Option<String> {
    if timestamp <= 0 {
        return None;
    }
    let millis = if timestamp > 10_000_000_000 {
        timestamp
    } else {
        timestamp.saturating_mul(1000)
    };
    chrono::DateTime::from_timestamp_millis(millis)
        .map(|dt| dt.with_timezone(&chrono::Local).format("%H:%M").to_string())
}

fn message_detail_lines(msg: &Message, width: usize) -> Vec<String> {
    let mut lines = vec![format!("uuid: {}", msg.uuid())];
    if let Some(ts) = concise_timestamp(msg.timestamp()) {
        lines.push(format!("time: {ts}"));
    }
    if let Some(reference) = message_primary_reference(msg) {
        lines.push(format!("ref: {reference}"));
    }
    let preview = cc_utils::messages::truncate_text(
        &message_copy_text(msg).replace('\n', " ⏎ "),
        width.saturating_sub(4).max(20),
    );
    if !preview.is_empty() {
        lines.push(format!("copy: {preview}"));
    }
    lines
}

// ── User messages ───────────────────────────────────────────────────────

fn render_user_message<'a>(
    msg: &cc_types::message::UserMessage,
    theme: &Theme,
    msg_index: usize,
    width: usize,
    render_context: &MessageRenderContext,
) -> Vec<Line<'a>> {
    let mut lines = Vec::new();

    let content_text = match &msg.content {
        MessageContent::Text(t) => t.clone(),
        MessageContent::Blocks(blocks) => {
            if let Some(tool_lines) = render_tool_result_user_message(
                msg,
                blocks,
                theme,
                msg_index,
                width,
                render_context,
            ) {
                return tool_lines;
            }
            if let Some(image_lines) = render_user_image_blocks(blocks, theme) {
                return image_lines;
            }
            blocks
                .iter()
                .filter_map(content_block_copy_text)
                .collect::<Vec<_>>()
                .join("\n")
        }
    };

    if let Some(rendered) =
        render_tagged_user_text(&content_text, msg.uuid, render_context, theme, width)
    {
        return rendered;
    }

    if content_text.trim() == "[Request interrupted by user]" {
        return vec![Line::from(Span::styled(
            "Interrupted by user",
            theme.warning,
        ))];
    }

    let routed = render_user_text_message(&content_text, theme);
    if routed.is_empty() {
        return Vec::new();
    }
    let content_text = routed
        .strip_prefix("You: ")
        .unwrap_or(routed.as_str())
        .to_string();

    // First line includes the "You: " prefix.
    let content_lines: Vec<&str> = content_text.lines().collect();
    if content_lines.is_empty() {
        lines.push(Line::from(vec![Span::styled("You: ", theme.user_name)]));
    } else {
        lines.push(Line::from(vec![
            Span::styled("You: ", theme.user_name),
            Span::raw(content_lines[0].to_string()),
        ]));
        for extra in &content_lines[1..] {
            lines.push(Line::from(format!("     {}", extra)));
        }
    }

    lines
}

fn render_user_image_blocks<'a>(blocks: &[ContentBlock], theme: &Theme) -> Option<Vec<Line<'a>>> {
    let images = blocks
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Image { source } => Some(image_reference(source)),
            _ => None,
        })
        .collect::<Vec<_>>();
    (!images.is_empty()).then(|| {
        images
            .into_iter()
            .map(|image| Line::from(Span::styled(image, theme.dim)))
            .collect()
    })
}

fn render_tagged_user_text<'a>(
    text: &str,
    uuid: uuid::Uuid,
    render_context: &MessageRenderContext,
    theme: &Theme,
    width: usize,
) -> Option<Vec<Line<'a>>> {
    let trimmed = text.trim();
    if !(trimmed.starts_with("<bash-stdout") || trimmed.starts_with("<bash-stderr")) {
        return None;
    }
    let output = extract_xmlish_body(trimmed).unwrap_or(trimmed);
    let rendered = render_user_bash_output_message_with_options(
        "bash",
        output,
        ShellOutputRenderOptions {
            width: width.max(20),
            expanded: render_context.lookups.latest_bash_output_uuid == Some(uuid),
            total_lines: Some(output.lines().count()),
            total_bytes: Some(output.len()),
            ..ShellOutputRenderOptions::default()
        },
    );
    Some(styled_text_lines(&rendered, theme.tool_result))
}

fn extract_xmlish_body(text: &str) -> Option<&str> {
    let start = text.find('>')? + 1;
    let end = text.rfind("</").unwrap_or(text.len());
    Some(text[start..end].trim())
}

fn render_tool_result_user_message<'a>(
    msg: &cc_types::message::UserMessage,
    blocks: &[ContentBlock],
    theme: &Theme,
    msg_index: usize,
    width: usize,
    render_context: &MessageRenderContext,
) -> Option<Vec<Line<'a>>> {
    let tool_result = blocks.iter().find_map(|block| match block {
        ContentBlock::ToolResult {
            tool_use_id,
            content,
            is_error,
        } => Some((tool_use_id, content, *is_error)),
        _ => None,
    })?;

    if let Some(tool_use) = render_context.tool_use(tool_result.0) {
        if tool_use.is_shell() {
            let output = msg
                .tool_use_result
                .as_deref()
                .map(str::to_string)
                .unwrap_or_else(|| tool_result_content_text(tool_result.1));
            let expanded = render_context.shell_expanded(msg_index, tool_result.0);
            let rendered = render_user_bash_output_message_with_options(
                &tool_use.command(),
                &output,
                ShellOutputRenderOptions {
                    width: width.max(20),
                    expanded,
                    total_lines: Some(output.lines().count()),
                    total_bytes: Some(output.len()),
                    ..ShellOutputRenderOptions::default()
                },
            );
            let style = if tool_result.2 {
                theme.error
            } else {
                theme.tool_result
            };
            return Some(styled_text_lines(&rendered, style));
        }
    }

    if let Some(preview) = msg.tool_use_result.as_deref() {
        if !tool_result.2 {
            if let Some(lines) = render_file_edit_preview(preview, theme) {
                return Some(lines);
            }
        }
    }

    let text = tool_result_content_text(tool_result.1);
    let mut block = ToolResultBlock::new(tool_result.0.clone(), tool_result.2, text.clone());
    block.tool_use_result = msg
        .tool_use_result
        .clone()
        .or_else(|| (!text.is_empty()).then_some(text));
    let mut lookups = UserToolResultLookups::new();
    for (id, record) in &render_context.lookups.tool_uses {
        lookups.tool_use_by_tool_use_id.insert(
            id.clone(),
            UserToolUseRecord {
                tool_name: record.tool_name.clone(),
                input: record.input.clone(),
            },
        );
    }
    let tools: cc_engine::types::tool::Tools = Vec::new();
    Some(render_user_tool_result_message(
        &block,
        &lookups,
        &tools,
        theme,
        render_context.options.verbose,
        width,
        render_context.options.is_transcript_mode,
    ))
}

fn render_file_edit_preview<'a>(preview: &str, theme: &Theme) -> Option<Vec<Line<'a>>> {
    let value = serde_json::from_str::<serde_json::Value>(preview).ok()?;
    if value.get("kind").and_then(|v| v.as_str()) != Some("file_edit") {
        return None;
    }
    let file_path = value.get("path").and_then(|v| v.as_str())?.to_string();
    let hunk_lines = value
        .get("hunk_lines")?
        .as_array()?
        .iter()
        .filter_map(|line| line.as_str().map(str::to_string))
        .collect::<Vec<_>>();
    if hunk_lines.is_empty() {
        return None;
    }

    let rendered = render_file_edit_tool_updated_message(&FileEditToolUpdatedView {
        file_path,
        hunk_lines,
        style: FileEditMessageStyle::Regular,
        verbose: true,
        preview_hint: None,
        width: 100,
        max_lines: 48,
    });
    Some(
        rendered
            .lines()
            .enumerate()
            .map(|(idx, line)| {
                let style = if idx == 0 {
                    theme.tool_name
                } else {
                    theme.tool_result
                };
                Line::from(Span::styled(line.to_string(), style))
            })
            .collect(),
    )
}

fn tool_result_content_text(content: &ToolResultContent) -> String {
    match content {
        ToolResultContent::Text(text) => text.clone(),
        ToolResultContent::Blocks(blocks) => blocks
            .iter()
            .filter_map(|block| match block {
                ContentBlock::Text { text } => Some(text.clone()),
                ContentBlock::Image { source } => Some(image_reference(source)),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

fn styled_text_lines<'a>(text: &str, style: Style) -> Vec<Line<'a>> {
    if text.is_empty() {
        return vec![Line::from(Span::styled("<empty tool output>", style))];
    }
    text.lines()
        .map(|line| Line::from(Span::styled(line.to_string(), style)))
        .collect()
}

fn plain_text_to_lines<'a>(text: &str, style: Style) -> Vec<Line<'a>> {
    if text.is_empty() {
        return Vec::new();
    }
    text.lines()
        .map(|line| Line::from(Span::styled(line.to_string(), style)))
        .collect()
}

fn api_error_display_text(text: &str) -> String {
    if let Some(error) = classify_assistant_text(text) {
        render_api_error(&error)
    } else {
        format!("Error occurred: {text}")
    }
}

// ── Assistant messages ──────────────────────────────────────────────────

fn render_assistant_message<'a>(
    msg: &cc_types::message::AssistantMessage,
    theme: &Theme,
    render_context: &MessageRenderContext,
) -> Vec<Line<'a>> {
    let mut lines = Vec::new();

    // Name prefix on the first line.
    let prefix = Span::styled("Claude: ", theme.assistant_name);
    let mut first_block = true;

    for block in &msg.content {
        match block {
            ContentBlock::Text { text } => {
                // If this is an API error message, use error classification
                // instead of standard markdown rendering.
                if msg.is_api_error_message {
                    let error_text = api_error_display_text(text);
                    let style = theme.error;
                    if first_block {
                        let mut spans = vec![prefix.clone()];
                        spans.push(Span::styled(error_text, style));
                        lines.push(Line::from(spans));
                    } else {
                        lines.push(Line::from(vec![
                            Span::raw("        "),
                            Span::styled(error_text, style),
                        ]));
                    }
                    first_block = false;
                    continue;
                }
                let md_lines = markdown_to_lines(text, theme);
                if md_lines.is_empty() {
                    if first_block {
                        lines.push(Line::from(vec![prefix.clone()]));
                    }
                } else {
                    for (i, md_line) in md_lines.into_iter().enumerate() {
                        if i == 0 && first_block {
                            // Prepend the "Claude: " prefix to the first line.
                            let mut spans = vec![prefix.clone()];
                            spans.extend(md_line.spans);
                            lines.push(Line::from(spans));
                        } else {
                            // Indent continuation lines to align with text after "Claude: "
                            let mut spans = vec![Span::raw("        ")];
                            spans.extend(md_line.spans);
                            lines.push(Line::from(spans));
                        }
                    }
                }
                first_block = false;
            }

            ContentBlock::ConnectorText { connector_text, .. } => {
                let md_lines = markdown_to_lines(connector_text, theme);
                if md_lines.is_empty() {
                    if first_block {
                        lines.push(Line::from(vec![prefix.clone()]));
                    }
                } else {
                    for (i, md_line) in md_lines.into_iter().enumerate() {
                        if i == 0 && first_block {
                            let mut spans = vec![prefix.clone()];
                            spans.extend(md_line.spans);
                            lines.push(Line::from(spans));
                        } else {
                            let mut spans = vec![Span::raw("        ")];
                            spans.extend(md_line.spans);
                            lines.push(Line::from(spans));
                        }
                    }
                }
                first_block = false;
            }

            ContentBlock::ToolUse { id, name, input } => {
                let input_json = serde_json::to_string(input).unwrap_or_else(|_| input.to_string());
                let state = tool_state_for_id(id, render_context);
                let rendered =
                    render_assistant_tool_use_message(name, &input_json, state, false, theme);
                for (i, line) in rendered.lines().enumerate() {
                    lines.push(Line::from(vec![
                        Span::raw(if first_block && i == 0 {
                            ""
                        } else {
                            "        "
                        }),
                        Span::styled(line.to_string(), theme.tool_name),
                    ]));
                }
                first_block = false;
            }

            ContentBlock::ServerToolUse { id, name, input } => {
                let input_json = serde_json::to_string(input).unwrap_or_else(|_| input.to_string());
                let state = tool_state_for_id(id, render_context);
                let rendered =
                    render_assistant_tool_use_message(name, &input_json, state, false, theme);
                for (i, line) in rendered.lines().enumerate() {
                    lines.push(Line::from(vec![
                        Span::raw(if first_block && i == 0 {
                            ""
                        } else {
                            "        "
                        }),
                        Span::styled(format!("server: {line}"), theme.tool_name),
                    ]));
                }
                first_block = false;
            }

            ContentBlock::ToolResult {
                tool_use_id: _,
                content,
                is_error,
            } => {
                let style = if *is_error {
                    theme.error
                } else {
                    theme.tool_result
                };
                let text = match content {
                    ToolResultContent::Text(t) => t.clone(),
                    ToolResultContent::Blocks(blocks) => blocks
                        .iter()
                        .map(|b| match b {
                            ContentBlock::Text { text } => text.clone(),
                            ContentBlock::ConnectorText { connector_text, .. } => {
                                connector_text.clone()
                            }
                            ContentBlock::Image { source } => image_reference(source),
                            _ => "[...]".to_string(),
                        })
                        .collect::<Vec<_>>()
                        .join("\n"),
                };
                let prefix_str = if *is_error { "  Error: " } else { "  Result: " };
                // Show first few lines of result.
                let result_lines: Vec<&str> = text.lines().take(5).collect();
                for (i, rl) in result_lines.iter().enumerate() {
                    if i == 0 {
                        lines.push(Line::from(vec![
                            Span::raw("        "),
                            Span::styled(prefix_str.to_string(), style),
                            Span::styled(rl.to_string(), style),
                        ]));
                    } else {
                        lines.push(Line::from(vec![
                            Span::raw("                  "),
                            Span::styled(rl.to_string(), style),
                        ]));
                    }
                }
                let total_lines = text.lines().count();
                if total_lines > 5 {
                    lines.push(Line::from(vec![
                        Span::raw("                  "),
                        Span::styled(format!("... {} more lines", total_lines - 5), theme.dim),
                    ]));
                }
                first_block = false;
            }

            ContentBlock::Thinking {
                thinking,
                signature: _,
            } => {
                let thinking_lines = render_assistant_thinking_lines(
                    &AssistantThinkingView {
                        thinking: thinking.clone(),
                        verbose: render_context.options.verbose,
                        is_transcript_mode: render_context.options.is_transcript_mode,
                    },
                    theme,
                );
                for (i, line) in thinking_lines.into_iter().enumerate() {
                    if first_block && i == 0 {
                        lines.push(line);
                    } else {
                        let mut spans = vec![Span::raw("        ")];
                        spans.extend(line.spans);
                        lines.push(Line::from(spans));
                    }
                }
                if !thinking.is_empty()
                    && (render_context.options.verbose || render_context.options.is_transcript_mode)
                {
                    first_block = false;
                }
            }

            ContentBlock::RedactedThinking { .. } => {
                if render_context.options.verbose || render_context.options.is_transcript_mode {
                    for (i, line) in crate::ui::messages::assistant_redacted_thinking_message::render_assistant_redacted_thinking_lines(theme).into_iter().enumerate() {
                        if first_block && i == 0 {
                            lines.push(line);
                        } else {
                            let mut spans = vec![Span::raw("        ")];
                            spans.extend(line.spans);
                            lines.push(Line::from(spans));
                        }
                    }
                    first_block = false;
                }
            }

            ContentBlock::Image { source } => {
                lines.push(Line::from(vec![
                    Span::raw(if first_block { "" } else { "        " }),
                    Span::styled(image_reference(source), theme.dim),
                ]));
                first_block = false;
            }
        }
    }

    // If there was no content at all, at least show the name.
    if lines.is_empty() {
        if msg.content.iter().all(|block| {
            matches!(
                block,
                ContentBlock::Thinking { .. } | ContentBlock::RedactedThinking { .. }
            )
        }) {
            return Vec::new();
        }
        lines.push(Line::from(vec![prefix]));
    }

    // Show cost if non-zero.
    if msg.cost_usd > 0.0 {
        lines.push(Line::from(vec![
            Span::raw("        "),
            Span::styled(format!("(${:.4})", msg.cost_usd), theme.dim),
        ]));
    }

    lines
}

fn tool_state_for_id(tool_use_id: &str, render_context: &MessageRenderContext) -> ToolUseState {
    if render_context
        .lookups
        .errored_tool_use_ids
        .contains(tool_use_id)
    {
        ToolUseState::Error
    } else if render_context
        .lookups
        .resolved_tool_use_ids
        .contains(tool_use_id)
    {
        ToolUseState::Resolved
    } else {
        ToolUseState::InProgress
    }
}

// ── System messages ─────────────────────────────────────────────────────

fn render_system_message<'a>(
    msg: &cc_types::message::SystemMessage,
    theme: &Theme,
) -> Vec<Line<'a>> {
    let mut lines = Vec::new();

    let (prefix, style) = match &msg.subtype {
        SystemSubtype::CompactBoundary { .. } => ("context compacted", theme.dim),
        SystemSubtype::MicrocompactBoundary { .. } => ("context microcompacted", theme.dim),
        SystemSubtype::ApiError { .. } => ("", theme.error),
        SystemSubtype::Informational { level } => match level {
            InfoLevel::Info => ("Info: ", theme.info),
            InfoLevel::Warning => ("Warning: ", theme.warning),
            InfoLevel::Error => ("Error: ", theme.error),
        },
        SystemSubtype::LocalCommand { .. } => ("$ ", theme.system_name),
        SystemSubtype::Warning => ("Warning: ", theme.warning),
    };

    if let SystemSubtype::ApiError {
        retry_attempt,
        max_retries: _,
        retry_in_ms,
        error,
    } = &msg.subtype
    {
        let detail = if msg.content.trim().is_empty() {
            error.message.as_str()
        } else {
            msg.content.trim()
        };
        let rendered = if *retry_in_ms > 0 {
            render_system_text_message(
                "api_error",
                &format!("retry_attempt={} {}", retry_attempt, detail),
                theme,
            )
        } else {
            render_system_text_message("api_error", detail, theme)
        };
        return plain_text_to_lines(&rendered, theme.error);
    }

    if matches!(&msg.subtype, SystemSubtype::MicrocompactBoundary { .. }) {
        return Vec::new();
    }

    if matches!(&msg.subtype, SystemSubtype::CompactBoundary { .. }) {
        return render_compact_boundary_lines(theme);
    }

    if matches!(
        &msg.subtype,
        SystemSubtype::CompactBoundary { .. } | SystemSubtype::MicrocompactBoundary { .. }
    ) {
        lines.push(Line::from(vec![Span::styled(
            compact_boundary_summary(&msg.subtype, prefix),
            style,
        )]));
    } else {
        let content_lines: Vec<&str> = msg.content.lines().collect();
        if content_lines.is_empty() {
            lines.push(Line::from(vec![Span::styled(prefix.to_string(), style)]));
        } else {
            lines.push(Line::from(vec![
                Span::styled(prefix.to_string(), style),
                Span::styled(content_lines[0].to_string(), style),
            ]));
            for extra in &content_lines[1..] {
                lines.push(Line::from(Span::styled(format!("  {}", extra), style)));
            }
        }
    }

    lines
}

// ── Progress messages ───────────────────────────────────────────────────

fn render_progress_message<'a>(
    msg: &cc_types::message::ProgressMessage,
    theme: &Theme,
) -> Vec<Line<'a>> {
    let data_summary = if msg.data.is_object() {
        msg.data
            .as_object()
            .and_then(|o| o.get("message"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    } else {
        msg.data.to_string()
    };

    vec![Line::from(vec![
        Span::styled("  ... ", theme.dim),
        Span::styled(data_summary, theme.dim),
    ])]
}

// ── Attachment messages ─────────────────────────────────────────────────

fn render_attachment_message<'a>(
    msg: &cc_types::message::AttachmentMessage,
    theme: &Theme,
) -> Vec<Line<'a>> {
    use cc_types::message::Attachment;
    let text = match &msg.attachment {
        Attachment::EditedTextFile { path } => format!("[edited: {}]", path),
        Attachment::QueuedCommand { prompt, .. } => render_attachment_helper(
            "queued_command",
            &serde_json::json!({ "prompt": prompt }).to_string(),
            theme,
        ),
        Attachment::MaxTurnsReached {
            max_turns,
            turn_count,
        } => format!("[max turns reached: {}/{}]", turn_count, max_turns),
        Attachment::StructuredOutput { .. } => "[structured output]".to_string(),
        Attachment::HookStoppedContinuation => "[hook stopped continuation]".to_string(),
        Attachment::NestedMemory { path, .. } => {
            render_attachment_helper("nested_memory", path, theme)
        }
        Attachment::SkillDiscovery { skills } => render_attachment_helper(
            "skill_discovery",
            &serde_json::to_string(skills).unwrap_or_else(|_| "[]".to_string()),
            theme,
        ),
    };
    vec![Line::from(Span::styled(text, theme.dim))]
}

// ── Utility ─────────────────────────────────────────────────────────────

/// Create an abbreviated string representation of a JSON value, capped at
/// `max_chars` characters.
#[cfg(test)]
fn abbreviate_json(value: &serde_json::Value, max_chars: usize) -> String {
    let full = match serde_json::to_string(value) {
        Ok(s) => s,
        Err(_) => value.to_string(),
    };
    if full.len() <= max_chars {
        full
    } else if max_chars > 3 {
        format!("{}...", &full[..max_chars - 3])
    } else {
        full[..max_chars].to_string()
    }
}
#[cfg(test)]
fn tool_input_summary(name: &str, input: &serde_json::Value, max_chars: usize) -> String {
    if let Some(primary) = tool_primary_input(name, input) {
        let json = abbreviate_json(input, max_chars);
        return format!("{primary} {json}").trim().to_string();
    }
    abbreviate_json(input, max_chars)
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

fn message_content_copy_text(content: &MessageContent) -> String {
    match content {
        MessageContent::Text(text) => strip_system_reminders(text),
        MessageContent::Blocks(blocks) => blocks
            .iter()
            .filter_map(content_block_copy_text)
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

fn message_content_reference(content: &MessageContent) -> Option<String> {
    match content {
        MessageContent::Text(_) => None,
        MessageContent::Blocks(blocks) => blocks.iter().find_map(content_block_reference),
    }
}

fn content_block_copy_text(block: &ContentBlock) -> Option<String> {
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

fn image_reference(source: &cc_types::message::ImageSource) -> String {
    format!(
        "[image: {}, {} chars]",
        source.media_type,
        source.data.len()
    )
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

fn compact_boundary_summary(subtype: &SystemSubtype, prefix: &str) -> String {
    match subtype {
        SystemSubtype::CompactBoundary {
            compact_metadata: Some(meta),
        } => format!(
            "--- {prefix}: {} → {} tokens ---",
            meta.pre_compact_token_count, meta.post_compact_token_count
        ),
        SystemSubtype::MicrocompactBoundary {
            microcompact_metadata: Some(meta),
        } => format!(
            "--- {prefix}: saved {} tokens from {} tool results ---",
            meta.tokens_saved,
            meta.compacted_tool_ids.len()
        ),
        _ => format!("--- {prefix} ---"),
    }
}

#[cfg(test)]
mod tests {
    use super::{message_copy_text, message_primary_reference, render_single_message};
    use crate::ui::diff::file_edit_diff::unified_hunk_lines_from_edit;
    use crate::ui::theme::Theme;
    use cc_types::message::{
        ApiErrorInfo, AssistantMessage, CompactMetadata, ContentBlock, ImageSource, Message,
        MessageContent, MicrocompactMetadata, SystemMessage, SystemSubtype, ToolResultContent,
        UserMessage,
    };
    use serde_json::json;

    #[test]
    fn renders_file_edit_tool_preview_from_tool_use_result() {
        let hunk_lines = unified_hunk_lines_from_edit(
            "src/lib.rs",
            "fn main() {\n    old_call();\n}\n",
            "fn main() {\n    new_call();\n}\n",
        );
        let preview = json!({
            "kind": "file_edit",
            "path": "src/lib.rs",
            "output": "Successfully replaced 1 occurrence(s) in src/lib.rs",
            "replacements": 1,
            "hunk_lines": hunk_lines,
        })
        .to_string();
        let message = Message::User(UserMessage {
            uuid: uuid::Uuid::new_v4(),
            timestamp: 0,
            role: "user".to_string(),
            content: MessageContent::Blocks(vec![ContentBlock::ToolResult {
                tool_use_id: "toolu_edit".to_string(),
                content: ToolResultContent::Text("The file was updated.".to_string()),
                is_error: false,
            }]),
            is_meta: true,
            tool_use_result: Some(preview),
            source_tool_assistant_uuid: None,
        });

        let rendered = render_single_message(&message, &Theme::default())
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        assert!(rendered.contains("Added 1 line, removed 1 line"));
        assert!(rendered.contains("file: src/lib.rs"));
        assert!(rendered.contains("old_call();"));
        assert!(rendered.contains("new_call();"));
    }

    #[test]
    fn messages_render_path_image_compact_and_interrupt_summaries() {
        let theme = Theme::default();
        let image = ImageSource {
            source_type: "base64".to_string(),
            media_type: "image/png".to_string(),
            data: "abcdef".to_string(),
        };
        let assistant = Message::Assistant(AssistantMessage {
            uuid: uuid::Uuid::new_v4(),
            timestamp: 1_700_000_000,
            role: "assistant".to_string(),
            content: vec![
                ContentBlock::ToolUse {
                    id: "toolu_read".to_string(),
                    name: "Read".to_string(),
                    input: json!({ "file_path": "src/main.rs" }),
                },
                ContentBlock::Image {
                    source: image.clone(),
                },
            ],
            usage: None,
            stop_reason: None,
            is_api_error_message: false,
            api_error: None,
            cost_usd: 0.0,
        });
        let rendered = lines_to_text(render_single_message(&assistant, &theme));

        assert!(rendered.contains("path=src/main.rs"));
        assert!(rendered.contains("[image: image/png, 6 chars]"));
        assert_eq!(
            message_primary_reference(&assistant).as_deref(),
            Some("path=src/main.rs")
        );
        assert!(message_copy_text(&assistant).contains("[image: image/png, 6 chars]"));

        let compact = Message::System(SystemMessage {
            uuid: uuid::Uuid::new_v4(),
            timestamp: 1_700_000_000,
            subtype: SystemSubtype::CompactBoundary {
                compact_metadata: Some(CompactMetadata {
                    pre_compact_token_count: 12_000,
                    post_compact_token_count: 4_000,
                    preserved_segment: None,
                }),
            },
            content: String::new(),
        });
        assert!(lines_to_text(render_single_message(&compact, &theme))
            .contains("✻ Conversation compacted (ctrl+o for history)"));

        let interrupted = Message::User(UserMessage {
            uuid: uuid::Uuid::new_v4(),
            timestamp: 0,
            role: "user".to_string(),
            content: MessageContent::Text("[Request interrupted by user]".to_string()),
            is_meta: false,
            tool_use_result: None,
            source_tool_assistant_uuid: None,
        });
        assert_eq!(
            lines_to_text(render_single_message(&interrupted, &theme)),
            "Interrupted by user"
        );
    }

    #[test]
    fn assistant_api_error_runtime_path_uses_error_occurred_prefix() {
        let message = Message::Assistant(AssistantMessage {
            uuid: uuid::Uuid::new_v4(),
            timestamp: 0,
            role: "assistant".to_string(),
            content: vec![ContentBlock::Text {
                text: "API error: Provider openrouter error (HTTP 429): rate limit exceeded"
                    .to_string(),
            }],
            usage: None,
            stop_reason: Some("error".to_string()),
            is_api_error_message: true,
            api_error: Some(
                "Provider openrouter error (HTTP 429): rate limit exceeded".to_string(),
            ),
            cost_usd: 0.0,
        });

        let rendered = lines_to_text(render_single_message(&message, &Theme::default()));

        assert!(rendered.contains(
            "Claude: Error occurred: API error: Provider openrouter error (HTTP 429): rate limit exceeded"
        ));
    }

    #[test]
    fn system_api_error_runtime_path_uses_error_occurred_prefix() {
        let message = Message::System(SystemMessage {
            uuid: uuid::Uuid::new_v4(),
            timestamp: 0,
            subtype: SystemSubtype::ApiError {
                retry_attempt: 1,
                max_retries: 3,
                retry_in_ms: 0,
                error: ApiErrorInfo {
                    status: Some(403),
                    message: "Provider proxy error (HTTP 403): forbidden".to_string(),
                },
            },
            content: String::new(),
        });

        let rendered = lines_to_text(render_single_message(&message, &Theme::default()));

        assert_eq!(
            rendered,
            "Error occurred: Provider proxy error (HTTP 403): forbidden"
        );
    }

    #[test]
    fn thinking_visibility_matches_prompt_and_transcript_modes() {
        let message = Message::Assistant(AssistantMessage {
            uuid: uuid::Uuid::new_v4(),
            timestamp: 0,
            role: "assistant".to_string(),
            content: vec![ContentBlock::Thinking {
                thinking: "inspect files".to_string(),
                signature: None,
            }],
            usage: None,
            stop_reason: None,
            is_api_error_message: false,
            api_error: None,
            cost_usd: 0.0,
        });

        let prompt = render_single_message(&message, &Theme::default());
        assert!(prompt.is_empty());

        let transcript_context = super::build_message_render_context_with_options(
            std::slice::from_ref(&message),
            None,
            false,
            super::MessageRenderOptions {
                verbose: false,
                is_transcript_mode: true,
                show_all_in_transcript: true,
            },
        );
        let transcript = super::render_single_message_with_context(
            &message,
            0,
            &Theme::default(),
            80,
            &transcript_context,
        );
        let rendered = lines_to_text(transcript);
        assert!(rendered.contains("∴ Thinking"));
        assert!(rendered.contains("inspect files"));
    }

    #[test]
    fn render_pipeline_collapses_read_search_and_hides_microcompact() {
        let read_id = "toolu_read".to_string();
        let grep_id = "toolu_grep".to_string();
        let messages = vec![
            Message::System(SystemMessage {
                uuid: uuid::Uuid::new_v4(),
                timestamp: 0,
                subtype: SystemSubtype::MicrocompactBoundary {
                    microcompact_metadata: Some(MicrocompactMetadata {
                        trigger: "test".to_string(),
                        pre_tokens: 100,
                        tokens_saved: 50,
                        compacted_tool_ids: vec![read_id.clone()],
                        cleared_attachment_uuids: Vec::new(),
                    }),
                },
                content: String::new(),
            }),
            Message::Assistant(AssistantMessage {
                uuid: uuid::Uuid::new_v4(),
                timestamp: 1,
                role: "assistant".to_string(),
                content: vec![
                    ContentBlock::ToolUse {
                        id: read_id.clone(),
                        name: "Read".to_string(),
                        input: json!({ "file_path": "src/lib.rs" }),
                    },
                    ContentBlock::ToolUse {
                        id: grep_id.clone(),
                        name: "Grep".to_string(),
                        input: json!({ "pattern": "fn main" }),
                    },
                ],
                usage: None,
                stop_reason: None,
                is_api_error_message: false,
                api_error: None,
                cost_usd: 0.0,
            }),
            Message::User(UserMessage {
                uuid: uuid::Uuid::new_v4(),
                timestamp: 2,
                role: "user".to_string(),
                content: MessageContent::Blocks(vec![ContentBlock::ToolResult {
                    tool_use_id: read_id,
                    content: ToolResultContent::Text("ok".to_string()),
                    is_error: false,
                }]),
                is_meta: true,
                tool_use_result: Some("ok".to_string()),
                source_tool_assistant_uuid: None,
            }),
            Message::User(UserMessage {
                uuid: uuid::Uuid::new_v4(),
                timestamp: 3,
                role: "user".to_string(),
                content: MessageContent::Blocks(vec![ContentBlock::ToolResult {
                    tool_use_id: grep_id,
                    content: ToolResultContent::Text("match".to_string()),
                    is_error: false,
                }]),
                is_meta: true,
                tool_use_result: Some("match".to_string()),
                source_tool_assistant_uuid: None,
            }),
        ];

        let context = super::build_message_render_context_with_options(
            &messages,
            None,
            false,
            super::MessageRenderOptions::default(),
        );
        let rendered = context
            .renderable_messages()
            .iter()
            .flat_map(|message| {
                super::render_renderable_message_with_context(
                    message,
                    0,
                    &Theme::default(),
                    80,
                    &context,
                )
            })
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        assert!(!rendered.contains("microcompact"));
        assert!(rendered.contains("Read 1 file"));
        assert!(rendered.contains("Searched for 1 pattern"));
    }

    #[test]
    fn tool_input_summary_prefers_primary_input_and_abbreviates_json() {
        let input = json!({
            "file_path": "src/main.rs",
            "content": "abcdefghijklmnopqrstuvwxyz"
        });

        let summary = super::tool_input_summary("Write", &input, 28);

        assert!(summary.starts_with("path=src/main.rs "));
        assert!(summary.ends_with("..."));
    }

    fn lines_to_text(lines: Vec<ratatui::text::Line<'_>>) -> String {
        lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}
