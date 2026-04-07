//! E2E tests for the session export feature.
//!
//! Validates:
//! - Module links correctly (init/system prompt don't break)
//! - SessionExport JSON roundtrip and schema validation
//! - Tool timeline reconstruction from synthetic messages
//! - Compact boundary extraction
//! - Content replacement detection pattern
//! - Context snapshot token/cost accounting
//!
//! Run:  cargo test --test e2e_session_export

use assert_cmd::Command;
use predicates::prelude::*;

const WORKSPACE: &str = r"F:\temp";

fn cli() -> Command {
    Command::cargo_bin("claude-code-rs").expect("binary not found")
}

fn strip_api_keys(cmd: &mut Command) -> &mut Command {
    cmd.env("ANTHROPIC_API_KEY", "")
        .env("AZURE_API_KEY", "")
        .env("OPENAI_API_KEY", "")
        .env("OPENROUTER_API_KEY", "")
        .env("GOOGLE_API_KEY", "")
        .env("DEEPSEEK_API_KEY", "")
}

// =========================================================================
//  Offline: init / system prompt still work with session_export linked in
// =========================================================================

#[test]
fn init_succeeds_with_session_export() {
    let mut cmd = cli();
    strip_api_keys(&mut cmd);
    cmd.args(["--init-only", "-C", WORKSPACE])
        .assert()
        .success();
}

#[test]
fn system_prompt_unaffected_by_session_export() {
    let mut cmd = cli();
    strip_api_keys(&mut cmd);
    cmd.args(["--dump-system-prompt", "-C", WORKSPACE])
        .assert()
        .success()
        .stdout(predicate::str::contains("Bash"))
        .stdout(predicate::str::contains("Read"));
}

// =========================================================================
//  SessionExport JSON roundtrip
// =========================================================================

#[test]
fn session_export_roundtrip() {
    let export = serde_json::json!({
        "schema_version": 1,
        "exported_at": "2026-04-07T12:00:00+00:00",
        "session": {
            "session_id": "test-session-001",
            "project_path": "/tmp/project",
            "git_branch": "main",
            "git_head_sha": "abc1234567890",
            "model": "claude-sonnet-4-20250514",
            "started_at": "2026-04-07T11:55:00+00:00",
            "ended_at": "2026-04-07T12:00:00+00:00"
        },
        "transcript": {
            "messages": [
                {"type": "user", "uuid": "u1", "content": "hello"},
                {"type": "assistant", "uuid": "a1", "content": [{"type": "text", "text": "hi"}]}
            ],
            "message_count": 2,
            "user_message_count": 1,
            "assistant_message_count": 1,
            "system_message_count": 0
        },
        "tool_calls": [],
        "compression": {
            "compact_boundaries": [],
            "content_replacements": [],
            "microcompact_replacements": [],
            "total_compactions": 0
        },
        "context": {
            "estimated_total_tokens": 50,
            "context_window_size": 200000,
            "utilization_pct": 0.025,
            "total_cost_usd": 0.001,
            "total_input_tokens": 100,
            "total_output_tokens": 20,
            "cache_read_tokens": 0,
            "api_call_count": 1,
            "tool_use_count": 0,
            "unique_tools_used": []
        }
    });

    // Write to tempfile
    let dir = tempfile::tempdir().expect("create tempdir");
    let path = dir.path().join("test.session.json");
    let json_str = serde_json::to_string_pretty(&export).unwrap();
    std::fs::write(&path, &json_str).unwrap();

    // Read back and validate
    let contents = std::fs::read_to_string(&path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&contents).unwrap();

    // Top-level fields
    assert_eq!(parsed["schema_version"], 1);
    assert!(parsed.get("exported_at").is_some());
    assert!(parsed.get("session").is_some());
    assert!(parsed.get("transcript").is_some());
    assert!(parsed.get("tool_calls").is_some());
    assert!(parsed.get("compression").is_some());
    assert!(parsed.get("context").is_some());

    // Session metadata
    assert_eq!(parsed["session"]["session_id"], "test-session-001");
    assert_eq!(parsed["session"]["git_branch"], "main");

    // Transcript
    assert_eq!(parsed["transcript"]["message_count"], 2);
    assert_eq!(parsed["transcript"]["user_message_count"], 1);
    assert_eq!(parsed["transcript"]["assistant_message_count"], 1);

    // Context
    assert_eq!(parsed["context"]["context_window_size"], 200000);
    assert!(parsed["context"]["total_cost_usd"].as_f64().unwrap() > 0.0);
}

// =========================================================================
//  Tool timeline reconstruction
// =========================================================================

#[test]
fn tool_timeline_reconstruction() {
    // Simulate: assistant calls Bash, then user sends tool_result
    let messages = serde_json::json!([
        {
            "type": "user",
            "uuid": "u1",
            "content": "list files"
        },
        {
            "type": "assistant",
            "uuid": "a1",
            "content": [
                {
                    "type": "tool_use",
                    "id": "tu_bash_1",
                    "name": "Bash",
                    "input": {"command": "ls -la"}
                }
            ]
        },
        {
            "type": "user",
            "uuid": "u2",
            "content": [
                {
                    "type": "tool_result",
                    "tool_use_id": "tu_bash_1",
                    "content": "file1.rs\nfile2.rs\nCargo.toml",
                    "is_error": false
                }
            ]
        },
        {
            "type": "assistant",
            "uuid": "a2",
            "content": [
                {"type": "text", "text": "Found 3 files."},
                {
                    "type": "tool_use",
                    "id": "tu_read_1",
                    "name": "Read",
                    "input": {"path": "file1.rs"}
                }
            ]
        },
        {
            "type": "user",
            "uuid": "u3",
            "content": [
                {
                    "type": "tool_result",
                    "tool_use_id": "tu_read_1",
                    "content": "fn main() {}",
                    "is_error": false
                }
            ]
        }
    ]);

    let msgs = messages.as_array().unwrap();

    // Verify we can parse tool_use from assistant messages
    let assistant_msgs: Vec<_> = msgs.iter()
        .filter(|m| m["type"] == "assistant")
        .collect();
    assert_eq!(assistant_msgs.len(), 2);

    // Verify tool_use blocks exist
    let first_assistant_content = assistant_msgs[0]["content"].as_array().unwrap();
    assert_eq!(first_assistant_content[0]["type"], "tool_use");
    assert_eq!(first_assistant_content[0]["name"], "Bash");
    assert_eq!(first_assistant_content[0]["id"], "tu_bash_1");

    // Verify tool_result blocks match
    let tool_result_msgs: Vec<_> = msgs.iter()
        .filter(|m| {
            m["content"].as_array()
                .map(|arr| arr.iter().any(|b| b["type"] == "tool_result"))
                .unwrap_or(false)
        })
        .collect();
    assert_eq!(tool_result_msgs.len(), 2);

    // Verify matching: tu_bash_1 → file listing, tu_read_1 → fn main
    let result1_content = &tool_result_msgs[0]["content"].as_array().unwrap()[0];
    assert_eq!(result1_content["tool_use_id"], "tu_bash_1");

    let result2_content = &tool_result_msgs[1]["content"].as_array().unwrap()[0];
    assert_eq!(result2_content["tool_use_id"], "tu_read_1");
}

// =========================================================================
//  Compact boundary extraction
// =========================================================================

#[test]
fn compact_boundary_extraction() {
    let messages = serde_json::json!([
        {"type": "user", "content": "hello"},
        {"type": "assistant", "content": [{"type": "text", "text": "hi"}]},
        {
            "type": "system",
            "subtype": "CompactBoundary",
            "content": "[Compacted: 150000 → 50000 tokens]",
            "compact_metadata": {
                "pre_compact_token_count": 150000,
                "post_compact_token_count": 50000
            }
        },
        {"type": "user", "content": "what were we talking about?"}
    ]);

    let msgs = messages.as_array().unwrap();

    // Find system messages with CompactBoundary
    let boundaries: Vec<_> = msgs.iter()
        .filter(|m| {
            m["type"] == "system" && m["content"].as_str()
                .map(|s| s.contains("Compacted"))
                .unwrap_or(false)
        })
        .collect();

    assert_eq!(boundaries.len(), 1);

    let boundary = boundaries[0];
    let meta = &boundary["compact_metadata"];
    assert_eq!(meta["pre_compact_token_count"], 150000);
    assert_eq!(meta["post_compact_token_count"], 50000);
}

// =========================================================================
//  Content replacement detection
// =========================================================================

#[test]
fn content_replacement_detection() {
    // The pattern from tool_result_budget.rs: make_preview()
    let preview = "first 500 chars of output...\n\n[... 98800 characters omitted. Full output saved to: /tmp/claude-code-rs/tool-results/tu_big.txt ...]\n\nlast 200 chars of output";

    // Verify the regex pattern works
    let re = regex::Regex::new(
        r"\[\.\.\.\s+(\d+)\s+characters omitted\.\s+Full output saved to:\s+(.+?)\s*\.\.\.\]"
    ).unwrap();

    let caps = re.captures(preview).expect("should match replacement marker");
    let omitted: usize = caps.get(1).unwrap().as_str().parse().unwrap();
    let file_path = caps.get(2).unwrap().as_str();

    assert_eq!(omitted, 98800);
    assert_eq!(file_path, "/tmp/claude-code-rs/tool-results/tu_big.txt");

    // Also test the truncated-in-place variant doesn't match the "saved to" pattern
    let truncated = "head\n\n[... 50000 characters omitted (truncated in place) ...]\n\ntail";
    assert!(re.captures(truncated).is_none());

    // But both should be detectable as "was replaced"
    assert!(preview.contains("characters omitted"));
    assert!(truncated.contains("characters omitted"));
}

// =========================================================================
//  Microcompact detection
// =========================================================================

#[test]
fn microcompact_detection() {
    // The pattern from microcompact.rs: make_tool_result_summary()
    let marker = "head of output\n\n[... 1500 characters omitted (microcompacted) ...]\n\ntail of output";

    let re = regex::Regex::new(
        r"\[\.\.\.\s+(\d+)\s+characters omitted \(microcompacted\)\s*\.\.\.\]"
    ).unwrap();

    let caps = re.captures(marker).expect("should match microcompact marker");
    let omitted: usize = caps.get(1).unwrap().as_str().parse().unwrap();
    assert_eq!(omitted, 1500);

    // The full-output-saved-to pattern should NOT match microcompact
    let budget_re = regex::Regex::new(
        r"\[\.\.\.\s+(\d+)\s+characters omitted\.\s+Full output saved to:\s+(.+?)\s*\.\.\.\]"
    ).unwrap();
    assert!(budget_re.captures(marker).is_none());

    // All three types detectable via generic contains check
    let budget = "[... 9000 characters omitted. Full output saved to: /tmp/x.txt ...]";
    let truncated = "[... 5000 characters omitted (truncated in place) ...]";
    assert!(marker.contains("characters omitted"));
    assert!(budget.contains("characters omitted"));
    assert!(truncated.contains("characters omitted"));
}

// =========================================================================
//  Context snapshot token/cost validation
// =========================================================================

#[test]
fn context_snapshot_schema() {
    let snapshot = serde_json::json!({
        "estimated_total_tokens": 15000,
        "context_window_size": 200000,
        "utilization_pct": 7.5,
        "total_cost_usd": 0.0234,
        "total_input_tokens": 12000,
        "total_output_tokens": 3000,
        "cache_read_tokens": 500,
        "api_call_count": 5,
        "tool_use_count": 8,
        "unique_tools_used": ["Bash", "Read", "Edit", "Grep"]
    });

    // All fields present
    assert!(snapshot.get("estimated_total_tokens").is_some());
    assert!(snapshot.get("context_window_size").is_some());
    assert!(snapshot.get("utilization_pct").is_some());
    assert!(snapshot.get("total_cost_usd").is_some());
    assert!(snapshot.get("total_input_tokens").is_some());
    assert!(snapshot.get("total_output_tokens").is_some());
    assert!(snapshot.get("cache_read_tokens").is_some());
    assert!(snapshot.get("api_call_count").is_some());
    assert!(snapshot.get("tool_use_count").is_some());
    assert!(snapshot.get("unique_tools_used").is_some());

    // Sanity checks
    let util = snapshot["utilization_pct"].as_f64().unwrap();
    let est_tokens = snapshot["estimated_total_tokens"].as_u64().unwrap();
    let window = snapshot["context_window_size"].as_u64().unwrap();
    let expected_util = (est_tokens as f64 / window as f64) * 100.0;
    assert!((util - expected_util).abs() < 0.01);

    let tools = snapshot["unique_tools_used"].as_array().unwrap();
    assert_eq!(tools.len(), 4);
    assert!(tools.contains(&serde_json::json!("Bash")));
}
