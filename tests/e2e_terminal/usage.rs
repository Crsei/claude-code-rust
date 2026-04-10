//! API token usage and cost tracking tests for headless IPC.
//!
//! Verifies that usage_update messages contain valid, non-zero token counts.

use crate::helpers::{collect_until, read_line_json, send_msg, spawn_headless, LIVE_TIMEOUT};

// =========================================================================
//  Live: usage_update field validation
// =========================================================================

/// After a simple chat response, usage_update should report non-zero tokens.
#[test]
#[ignore]
fn usage_update_has_nonzero_tokens() {
    let (mut child, mut stdin, mut stdout) = spawn_headless(
        &["-C", r"F:\temp", "--permission-mode", "bypass"],
        false,
    );

    let ready = read_line_json(&mut stdout, LIVE_TIMEOUT);
    assert_eq!(ready["type"], "ready");

    send_msg(
        &mut stdin,
        &serde_json::json!({
            "type": "submit_prompt",
            "text": "Say exactly: TOKEN_TEST_OK",
            "id": "usage-001"
        }),
    );

    let messages = collect_until(
        &mut stdout,
        |msg| msg["type"] == "usage_update",
        LIVE_TIMEOUT,
    );

    let usage = messages
        .iter()
        .find(|m| m["type"] == "usage_update")
        .expect("should have a usage_update message");

    let input_tokens = usage["input_tokens"]
        .as_u64()
        .expect("input_tokens should be a number");
    let output_tokens = usage["output_tokens"]
        .as_u64()
        .expect("output_tokens should be a number");
    let cost_usd = usage["cost_usd"]
        .as_f64()
        .expect("cost_usd should be a number");

    assert!(
        input_tokens > 0,
        "input_tokens should be > 0, got: {}",
        input_tokens
    );
    assert!(
        output_tokens > 0,
        "output_tokens should be > 0, got: {}",
        output_tokens
    );
    assert!(
        cost_usd >= 0.0,
        "cost_usd should be >= 0.0, got: {}",
        cost_usd
    );

    send_msg(&mut stdin, &serde_json::json!({"type": "quit"}));
    let status = child.wait().expect("wait");
    assert!(status.success());
}

/// After a tool-use response, usage_update should report higher token counts
/// than a simple chat (system prompt + tool schemas consume tokens).
#[test]
#[ignore]
fn usage_update_tool_use_has_higher_tokens() {
    let (mut child, mut stdin, mut stdout) = spawn_headless(
        &["-C", r"F:\temp", "--permission-mode", "bypass"],
        false,
    );

    let ready = read_line_json(&mut stdout, LIVE_TIMEOUT);
    assert_eq!(ready["type"], "ready");

    send_msg(
        &mut stdin,
        &serde_json::json!({
            "type": "submit_prompt",
            "text": "Use the Bash tool to run: echo USAGE_TOOL_TEST",
            "id": "usage-002"
        }),
    );

    let messages = collect_until(
        &mut stdout,
        |msg| msg["type"] == "usage_update",
        LIVE_TIMEOUT,
    );

    let usage = messages
        .iter()
        .find(|m| m["type"] == "usage_update")
        .expect("should have a usage_update message");

    let input_tokens = usage["input_tokens"].as_u64().unwrap_or(0);
    let output_tokens = usage["output_tokens"].as_u64().unwrap_or(0);

    // Tool-use sessions include tool schemas in system prompt,
    // so input tokens should be substantial (>100 at minimum)
    assert!(
        input_tokens > 100,
        "tool-use session input_tokens should be > 100, got: {}",
        input_tokens
    );
    assert!(
        output_tokens > 0,
        "output_tokens should be > 0, got: {}",
        output_tokens
    );

    send_msg(&mut stdin, &serde_json::json!({"type": "quit"}));
    let status = child.wait().expect("wait");
    assert!(status.success());
}

/// Two consecutive prompts should produce two usage_update messages,
/// and the second should have cumulative (higher) token counts.
#[test]
#[ignore]
fn usage_update_cumulative_across_turns() {
    let (mut child, mut stdin, mut stdout) = spawn_headless(
        &["-C", r"F:\temp", "--permission-mode", "bypass"],
        false,
    );

    let ready = read_line_json(&mut stdout, LIVE_TIMEOUT);
    assert_eq!(ready["type"], "ready");

    // First prompt
    send_msg(
        &mut stdin,
        &serde_json::json!({
            "type": "submit_prompt",
            "text": "Say: FIRST",
            "id": "cumul-001"
        }),
    );
    let msgs1 = collect_until(
        &mut stdout,
        |msg| msg["type"] == "usage_update",
        LIVE_TIMEOUT,
    );
    let usage1 = msgs1
        .iter()
        .find(|m| m["type"] == "usage_update")
        .expect("usage_update for turn 1");
    let tokens1 = usage1["input_tokens"].as_u64().unwrap_or(0)
        + usage1["output_tokens"].as_u64().unwrap_or(0);

    // Second prompt
    send_msg(
        &mut stdin,
        &serde_json::json!({
            "type": "submit_prompt",
            "text": "Say: SECOND",
            "id": "cumul-002"
        }),
    );
    let msgs2 = collect_until(
        &mut stdout,
        |msg| msg["type"] == "usage_update",
        LIVE_TIMEOUT,
    );
    let usage2 = msgs2
        .iter()
        .find(|m| m["type"] == "usage_update")
        .expect("usage_update for turn 2");
    let tokens2 = usage2["input_tokens"].as_u64().unwrap_or(0)
        + usage2["output_tokens"].as_u64().unwrap_or(0);

    // Second turn should have more cumulative tokens (conversation grows)
    assert!(
        tokens2 > tokens1,
        "turn 2 cumulative tokens ({}) should exceed turn 1 ({})",
        tokens2,
        tokens1
    );

    send_msg(&mut stdin, &serde_json::json!({"type": "quit"}));
    let status = child.wait().expect("wait");
    assert!(status.success());
}

/// After a chat response, usage_update.cost_usd should be > 0 when pricing is configured.
/// This validates the full pipeline: API → StreamAccumulator → pricing → IPC → cost_usd.
#[test]
#[ignore]
fn usage_update_cost_usd_positive() {
    let (mut child, mut stdin, mut stdout) = spawn_headless(
        &["-C", r"F:\temp", "--permission-mode", "bypass"],
        false,
    );

    let ready = read_line_json(&mut stdout, LIVE_TIMEOUT);
    assert_eq!(ready["type"], "ready");

    send_msg(
        &mut stdin,
        &serde_json::json!({
            "type": "submit_prompt",
            "text": "Say exactly: COST_TEST_OK",
            "id": "cost-001"
        }),
    );

    let messages = collect_until(
        &mut stdout,
        |msg| msg["type"] == "usage_update",
        LIVE_TIMEOUT,
    );

    let usage = messages
        .iter()
        .find(|m| m["type"] == "usage_update")
        .expect("should have a usage_update message");

    let cost_usd = usage["cost_usd"]
        .as_f64()
        .expect("cost_usd should be a number");
    let input_tokens = usage["input_tokens"]
        .as_u64()
        .expect("input_tokens should be a number");
    let output_tokens = usage["output_tokens"]
        .as_u64()
        .expect("output_tokens should be a number");

    assert!(
        input_tokens > 0,
        "input_tokens should be > 0, got: {}",
        input_tokens
    );
    assert!(
        output_tokens > 0,
        "output_tokens should be > 0, got: {}",
        output_tokens
    );
    // With pricing configured (MODEL_INPUT_PRICE/MODEL_OUTPUT_PRICE in .env or
    // built-in pricing table), cost_usd must be positive.
    assert!(
        cost_usd > 0.0,
        "cost_usd should be > 0.0 when pricing is configured, got: {}. \
         Ensure MODEL_INPUT_PRICE/MODEL_OUTPUT_PRICE are set in .env \
         or the model has built-in pricing.",
        cost_usd
    );

    // Sanity: cost should be reasonable (< $1 for a tiny prompt)
    assert!(
        cost_usd < 1.0,
        "cost_usd seems unreasonably high for a tiny prompt: {}",
        cost_usd
    );

    send_msg(&mut stdin, &serde_json::json!({"type": "quit"}));
    let status = child.wait().expect("wait");
    assert!(status.success());
}

/// usage_update should include all three fields: input_tokens, output_tokens, cost_usd.
#[test]
#[ignore]
fn usage_update_schema_complete() {
    let (mut child, mut stdin, mut stdout) = spawn_headless(
        &["-C", r"F:\temp", "--permission-mode", "bypass"],
        false,
    );

    let ready = read_line_json(&mut stdout, LIVE_TIMEOUT);
    assert_eq!(ready["type"], "ready");

    send_msg(
        &mut stdin,
        &serde_json::json!({
            "type": "submit_prompt",
            "text": "Hi",
            "id": "schema-001"
        }),
    );

    let messages = collect_until(
        &mut stdout,
        |msg| msg["type"] == "usage_update",
        LIVE_TIMEOUT,
    );

    let usage = messages
        .iter()
        .find(|m| m["type"] == "usage_update")
        .expect("should have usage_update");

    // All fields must be present and correct types
    assert!(
        usage.get("input_tokens").is_some(),
        "missing input_tokens: {:?}",
        usage
    );
    assert!(
        usage.get("output_tokens").is_some(),
        "missing output_tokens: {:?}",
        usage
    );
    assert!(
        usage.get("cost_usd").is_some(),
        "missing cost_usd: {:?}",
        usage
    );

    // Types: integers for tokens, float for cost
    assert!(
        usage["input_tokens"].is_u64() || usage["input_tokens"].is_i64(),
        "input_tokens should be integer: {:?}",
        usage["input_tokens"]
    );
    assert!(
        usage["output_tokens"].is_u64() || usage["output_tokens"].is_i64(),
        "output_tokens should be integer: {:?}",
        usage["output_tokens"]
    );
    assert!(
        usage["cost_usd"].is_f64() || usage["cost_usd"].is_i64(),
        "cost_usd should be numeric: {:?}",
        usage["cost_usd"]
    );

    send_msg(&mut stdin, &serde_json::json!({"type": "quit"}));
    let status = child.wait().expect("wait");
    assert!(status.success());
}
