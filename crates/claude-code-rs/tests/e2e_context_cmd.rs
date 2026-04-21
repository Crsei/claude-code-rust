//! E2E tests for the `/context` slash command (issue #38).
//!
//! `claude-code-rs` is a binary-only crate (no `lib.rs`), so these tests
//! use the public API of the `cc-compact` crate and source-file checks
//! to assert the wiring of the new `analyze_context_usage` service.

use std::fs;
use std::path::Path;

use cc_compact::context_analysis::{
    analyze_context_usage, ContextAnalysis, ContextAnalysisInput,
};
use cc_types::message::{
    AssistantMessage, ContentBlock, Message, MessageContent, UserMessage,
};
use uuid::Uuid;

fn user(text: &str) -> Message {
    Message::User(UserMessage {
        uuid: Uuid::new_v4(),
        timestamp: 0,
        role: "user".into(),
        content: MessageContent::Text(text.into()),
        is_meta: false,
        tool_use_result: None,
        source_tool_assistant_uuid: None,
    })
}

fn assistant(text: &str) -> Message {
    Message::Assistant(AssistantMessage {
        uuid: Uuid::new_v4(),
        timestamp: 0,
        role: "assistant".into(),
        content: vec![ContentBlock::Text { text: text.into() }],
        usage: None,
        stop_reason: Some("end_turn".into()),
        is_api_error_message: false,
        api_error: None,
        cost_usd: 0.0,
    })
}

#[test]
fn context_analysis_is_reachable_across_crate_boundary() {
    let _fn: fn(ContextAnalysisInput<'_>) -> ContextAnalysis = analyze_context_usage;
}

#[test]
fn report_includes_all_seven_canonical_categories() {
    let report = analyze_context_usage(ContextAnalysisInput {
        messages: &[],
        model: "claude-sonnet-4-20250514",
        ..Default::default()
    });
    let labels: Vec<&str> = report.categories.iter().map(|c| c.label.as_str()).collect();
    for expected in ["messages", "system prompt", "skills", "files cached", "tools schema", "hook results", "free"] {
        assert!(labels.contains(&expected));
    }
}

#[test]
fn total_used_plus_free_never_exceeds_window() {
    let messages = vec![
        user("hello world"),
        assistant("hi"),
        user(&"x".repeat(4_000)),
    ];
    let report = analyze_context_usage(ContextAnalysisInput {
        messages: &messages,
        system_prompt: Some(&"sys".repeat(500)),
        skills_manifest: Some("skill-a"),
        tools_schema: Some(&"tools".repeat(200)),
        hook_results: Some("{}"),
        cached_files_chars: 2_000,
        model: "claude-sonnet-4-20250514",
    });
    let free = report.categories.iter().find(|c| c.label == "free").unwrap().tokens;
    let capped = report.total_used.min(report.context_window);
    assert!(capped + free <= report.context_window);
    assert!(report.total_percent <= 100.0);
}

#[test]
fn categories_sorted_descending_with_free_pinned_last() {
    let report = analyze_context_usage(ContextAnalysisInput {
        messages: &[user("x")],
        system_prompt: Some(&"abc".repeat(1_000)),
        tools_schema: Some("t"),
        model: "claude-sonnet-4-20250514",
        ..Default::default()
    });
    assert_eq!(report.categories.last().unwrap().label, "free");
    let non_free: Vec<_> = report.categories.iter().filter(|c| c.label != "free").collect();
    for pair in non_free.windows(2) {
        assert!(pair[0].tokens >= pair[1].tokens);
    }
}

#[test]
fn json_shape_is_stable_for_headless_callers() {
    let report = analyze_context_usage(ContextAnalysisInput {
        messages: &[user("hi")],
        model: "claude-sonnet-4-20250514",
        ..Default::default()
    });
    let json = serde_json::to_value(&report).expect("serialise analysis");
    for key in ["model", "context_window", "total_used", "total_percent", "compacted", "messages_in", "messages_out", "categories"] {
        assert!(json.get(key).is_some());
    }
    for cat in json.get("categories").unwrap().as_array().unwrap() {
        assert!(cat.get("label").is_some());
        assert!(cat.get("tokens").is_some());
        assert!(cat.get("percent").is_some());
    }
}

fn command_file() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src").join("commands").join("context.rs");
    fs::read_to_string(&path).expect("read commands/context.rs")
}

#[test]
fn context_handler_delegates_to_service() {
    let text = command_file();
    assert!(text.contains("analyze_context_usage"));
    assert!(text.contains("ContextAnalysisInput"));
}

#[test]
fn context_handler_offers_json_subcommand() {
    let text = command_file();
    assert!(text.contains("\"json\""));
    assert!(text.contains("\"raw\""));
}

#[test]
fn context_command_is_still_registered() {
    let mod_rs = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src").join("commands").join("mod.rs");
    let text = fs::read_to_string(&mod_rs).expect("read commands/mod.rs");
    assert!(text.contains("name: \"context\""));
    assert!(text.contains("context::ContextHandler"));
}
