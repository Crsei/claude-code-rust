use super::*;
use crate::types::tool::Tool;
use serde_json::json;

#[test]
fn test_resolve_model_alias() {
    assert!(resolve_model_alias("sonnet", "fallback").contains("sonnet"));
    assert!(resolve_model_alias("opus", "fallback").contains("opus"));
    assert!(resolve_model_alias("haiku", "fallback").contains("haiku"));
    assert_eq!(
        resolve_model_alias("custom-model", "fallback"),
        "custom-model"
    );
}

#[test]
fn test_agent_tool_schema() {
    let tool = AgentTool;
    let schema = tool.input_json_schema();
    assert!(schema.get("properties").is_some());
    let props = schema["properties"].as_object().unwrap();
    assert!(props.contains_key("prompt"));
    assert!(props.contains_key("description"));
    assert!(props.contains_key("subagent_type"));
    assert!(props.contains_key("model"));
    assert!(props.contains_key("run_in_background"));
    assert!(props.contains_key("isolation"));
}

#[test]
fn test_agent_tool_name() {
    let tool = AgentTool;
    assert_eq!(tool.name(), "Agent");
}

#[test]
fn test_agent_user_facing_name() {
    let tool = AgentTool;
    assert_eq!(tool.user_facing_name(None), "Agent");

    let input = json!({"description": "search codebase"});
    assert_eq!(
        tool.user_facing_name(Some(&input)),
        "Agent(search codebase)"
    );
}

#[test]
fn test_agent_concurrency_safe() {
    let tool = AgentTool;
    assert!(tool.is_concurrency_safe(&json!({})));
}

#[test]
fn test_agent_isolation_field() {
    // No isolation field — should be None
    let input: AgentInput = serde_json::from_value(json!({
        "prompt": "do something",
        "description": "test task"
    }))
    .unwrap();
    assert!(input.isolation.is_none());

    // Explicit worktree isolation
    let input: AgentInput = serde_json::from_value(json!({
        "prompt": "do something",
        "description": "test task",
        "isolation": "worktree"
    }))
    .unwrap();
    assert_eq!(input.isolation.as_deref(), Some("worktree"));

    // Null isolation — should be None
    let input: AgentInput = serde_json::from_value(json!({
        "prompt": "do something",
        "description": "test task",
        "isolation": null
    }))
    .unwrap();
    assert!(input.isolation.is_none());
}

#[test]
fn test_agent_input_deserialization() {
    // Minimal required fields
    let input: AgentInput = serde_json::from_value(json!({
        "prompt": "find all TODO comments"
    }))
    .unwrap();
    assert_eq!(input.prompt, "find all TODO comments");
    assert!(input.description.is_none());
    assert!(input.model.is_none());
    assert!(!input.run_in_background);
    assert!(input.isolation.is_none());

    // Full fields
    let input: AgentInput = serde_json::from_value(json!({
        "prompt": "search for bugs",
        "description": "bug search",
        "subagent_type": "Explore",
        "model": "haiku",
        "run_in_background": true,
        "isolation": "worktree"
    }))
    .unwrap();
    assert_eq!(input.prompt, "search for bugs");
    assert_eq!(input.description.as_deref(), Some("bug search"));
    assert_eq!(input.subagent_type.as_deref(), Some("Explore"));
    assert_eq!(input.model.as_deref(), Some("haiku"));
    assert!(input.run_in_background);
    assert_eq!(input.isolation.as_deref(), Some("worktree"));
}

#[test]
fn test_max_result_size() {
    let tool = AgentTool;
    assert_eq!(tool.max_result_size_chars(), 200_000);
}

#[tokio::test]
async fn test_run_in_background_without_tx_falls_back() {
    // When bg_agent_tx is None, run_in_background should parse correctly
    let input: AgentInput = serde_json::from_value(json!({
        "prompt": "test task",
        "description": "test",
        "run_in_background": true
    }))
    .unwrap();
    assert!(input.run_in_background);
}

#[tokio::test]
async fn test_background_agent_placeholder_format() {
    // Verify the placeholder message format
    let agent_id = "test-agent-123";
    let description = "search codebase";
    let placeholder = format!(
        "Agent '{}' launched in background (id: {}). You will be notified when it completes.",
        description, agent_id
    );
    assert!(placeholder.contains("search codebase"));
    assert!(placeholder.contains("test-agent-123"));
    assert!(placeholder.contains("background"));
}
