use super::builders;
use super::compression;
use super::compression::detect_microcompact;
use super::*;
use crate::types::message::*;
use uuid::Uuid;

fn make_user_msg(text: &str) -> Message {
    Message::User(UserMessage {
        uuid: Uuid::new_v4(),
        timestamp: 1700000000000,
        role: "user".into(),
        content: MessageContent::Text(text.into()),
        is_meta: false,
        tool_use_result: None,
        source_tool_assistant_uuid: None,
    })
}

fn make_assistant_msg(text: &str) -> Message {
    Message::Assistant(AssistantMessage {
        uuid: Uuid::new_v4(),
        timestamp: 1700000001000,
        role: "assistant".into(),
        content: vec![ContentBlock::Text { text: text.into() }],
        usage: Some(Usage {
            input_tokens: 100,
            output_tokens: 50,
            cache_read_input_tokens: 10,
            cache_creation_input_tokens: 0,
        }),
        stop_reason: Some("end_turn".into()),
        is_api_error_message: false,
        api_error: None,
        cost_usd: 0.001,
    })
}

fn make_assistant_with_tool_use(tool_use_id: &str, tool_name: &str) -> Message {
    Message::Assistant(AssistantMessage {
        uuid: Uuid::new_v4(),
        timestamp: 1700000002000,
        role: "assistant".into(),
        content: vec![ContentBlock::ToolUse {
            id: tool_use_id.into(),
            name: tool_name.into(),
            input: serde_json::json!({"command": "ls"}),
        }],
        usage: Some(Usage {
            input_tokens: 200,
            output_tokens: 30,
            cache_read_input_tokens: 0,
            cache_creation_input_tokens: 0,
        }),
        stop_reason: Some("tool_use".into()),
        is_api_error_message: false,
        api_error: None,
        cost_usd: 0.002,
    })
}

fn make_tool_result_msg(tool_use_id: &str, result: &str, is_error: bool) -> Message {
    Message::User(UserMessage {
        uuid: Uuid::new_v4(),
        timestamp: 1700000003000,
        role: "user".into(),
        content: MessageContent::Blocks(vec![ContentBlock::ToolResult {
            tool_use_id: tool_use_id.into(),
            content: ToolResultContent::Text(result.into()),
            is_error,
        }]),
        is_meta: true,
        tool_use_result: None,
        source_tool_assistant_uuid: None,
    })
}

fn make_compact_boundary(pre: u64, post: u64) -> Message {
    Message::System(SystemMessage {
        uuid: Uuid::new_v4(),
        timestamp: 1700000004000,
        subtype: SystemSubtype::CompactBoundary {
            compact_metadata: Some(CompactMetadata {
                pre_compact_token_count: pre,
                post_compact_token_count: post,
            }),
        },
        content: format!("[Compacted: {} \u{2192} {} tokens]", pre, post),
    })
}

#[test]
fn test_reconstruct_tool_timeline_basic() {
    let messages = vec![
        make_user_msg("hello"),
        make_assistant_with_tool_use("tu_1", "Bash"),
        make_tool_result_msg("tu_1", "file1.rs\nfile2.rs", false),
        make_assistant_msg("Here are the files."),
    ];

    let timeline = reconstruct_tool_timeline(&messages);
    assert_eq!(timeline.len(), 1);
    assert_eq!(timeline[0].tool_name, "Bash");
    assert_eq!(timeline[0].tool_use_id, "tu_1");
    assert!(timeline[0].result.is_some());
    assert!(!timeline[0].is_error);
    assert!(!timeline[0].was_content_replaced);
}

#[test]
fn test_reconstruct_multiple_tool_calls() {
    let messages = vec![
        make_user_msg("read two files"),
        make_assistant_with_tool_use("tu_1", "Read"),
        make_tool_result_msg("tu_1", "contents of file1", false),
        make_assistant_with_tool_use("tu_2", "Read"),
        make_tool_result_msg("tu_2", "contents of file2", false),
        make_assistant_msg("Done."),
    ];

    let timeline = reconstruct_tool_timeline(&messages);
    assert_eq!(timeline.len(), 2);
    assert_eq!(timeline[0].sequence, 0);
    assert_eq!(timeline[1].sequence, 1);
}

#[test]
fn test_reconstruct_unmatched_tool_use() {
    let messages = vec![
        make_user_msg("do something"),
        make_assistant_with_tool_use("tu_orphan", "Bash"),
        // No tool_result — user interrupted
        make_user_msg("never mind"),
    ];

    let timeline = reconstruct_tool_timeline(&messages);
    assert_eq!(timeline.len(), 1);
    assert!(timeline[0].result.is_none());
    assert_eq!(timeline[0].tool_use_id, "tu_orphan");
}

#[test]
fn test_extract_compact_boundaries() {
    let messages = vec![
        make_user_msg("hello"),
        make_assistant_msg("hi"),
        make_compact_boundary(150000, 50000),
        make_user_msg("after compact"),
    ];

    let compression = extract_compression_events(&messages);
    assert_eq!(compression.compact_boundaries.len(), 1);
    assert_eq!(compression.total_compactions, 1);
    assert_eq!(
        compression.compact_boundaries[0].pre_compact_tokens,
        Some(150000)
    );
    assert_eq!(
        compression.compact_boundaries[0].post_compact_tokens,
        Some(50000)
    );
}

#[test]
fn test_detect_content_replacement() {
    let marker = "head text\n\n[... 50000 characters omitted. Full output saved to: /tmp/tool-results/tu_big.txt ...]\n\ntail text";
    let content = ToolResultContent::Text(marker.into());
    let record = detect_content_replacement("tu_big", &content);
    assert!(record.is_some());
    let r = record.unwrap();
    assert_eq!(r.tool_use_id, "tu_big");
    assert_eq!(r.original_size_hint, Some(50000));
    assert_eq!(r.file_path.as_deref(), Some("/tmp/tool-results/tu_big.txt"));
}

#[test]
fn test_detect_no_replacement() {
    let content = ToolResultContent::Text("normal output".into());
    assert!(detect_content_replacement("tu_1", &content).is_none());
}

#[test]
fn test_detect_microcompact() {
    let marker = "head text\n\n[... 1500 characters omitted (microcompacted) ...]\n\ntail text";
    let content = ToolResultContent::Text(marker.into());
    let record = detect_microcompact("tu_mc", &content);
    assert!(record.is_some());
    let r = record.unwrap();
    assert_eq!(r.tool_use_id, "tu_mc");
    assert_eq!(r.omitted_chars, 1500);
}

#[test]
fn test_detect_microcompact_not_present() {
    let content = ToolResultContent::Text("normal output".into());
    assert!(detect_microcompact("tu_1", &content).is_none());
}

#[test]
fn test_detect_content_replacement_in_result_microcompact() {
    let content = ToolResultContent::Text(
        "head\n\n[... 800 characters omitted (microcompacted) ...]\n\ntail".into(),
    );
    assert!(compression::detect_content_replacement_in_result(&content));
}

#[test]
fn test_build_context_snapshot() {
    let messages = vec![
        make_user_msg("hello"),
        make_assistant_with_tool_use("tu_1", "Bash"),
        make_tool_result_msg("tu_1", "output", false),
        make_assistant_msg("done"),
    ];

    let ctx = build_context_snapshot(&messages);
    assert!(ctx.estimated_total_tokens > 0);
    assert_eq!(ctx.context_window_size, 200_000);
    assert!(ctx.utilization_pct >= 0.0);
    assert_eq!(ctx.tool_use_count, 1);
    assert!(ctx.unique_tools_used.contains(&"Bash".to_string()));
    assert!(ctx.total_cost_usd > 0.0);
    assert_eq!(ctx.api_call_count, 2); // assistant with tool_use + assistant with text
}

#[test]
fn test_build_transcript_data() {
    let messages = vec![
        make_user_msg("hello"),
        make_assistant_msg("hi"),
        make_compact_boundary(100000, 30000),
    ];

    let transcript = builders::build_transcript_data(&messages);
    assert_eq!(transcript.message_count, 3);
    assert_eq!(transcript.user_message_count, 1);
    assert_eq!(transcript.assistant_message_count, 1);
    assert_eq!(transcript.system_message_count, 1);
    assert_eq!(transcript.messages.len(), 3);
}

#[test]
fn test_export_dir() {
    let dir = get_export_dir();
    assert!(dir.to_string_lossy().contains("exports"));
}

#[test]
fn test_format_ts_millis() {
    let ts = format_ts_millis(1700000000000);
    assert!(ts.contains("2023"));
}
