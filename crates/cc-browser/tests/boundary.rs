#[test]
fn browser_crate_owns_browser_tool_detection_and_rendering() {
    let (server, action) =
        cc_browser::detection::extract_browser_action("mcp__claude-in-chrome__navigate").unwrap();
    assert_eq!(server, "claude-in-chrome");
    assert_eq!(action, "navigate");

    let summary = cc_browser::tool_rendering::short_summary(
        "navigate",
        r#"{"url":"https://example.com"}"#,
        false,
    );
    assert!(summary.contains("navigated"));
    assert!(summary.contains("https://example.com"));
}

#[test]
fn browser_bridge_uses_mcp_protocol_without_reversing_dependency() {
    assert_eq!(cc_mcp::PROTOCOL_VERSION, "2024-11-05");

    let config: cc_mcp::McpServerConfig = serde_json::from_value(serde_json::json!({
        "type": "stdio",
        "command": "claude-browser",
        "browserMcp": true
    }))
    .unwrap();
    assert_eq!(config.browser_mcp, Some(true));
}
