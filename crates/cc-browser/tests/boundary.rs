use std::fs;
use std::path::Path;

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
fn browser_bridge_uses_mcp_contract_without_runtime_dependency() {
    assert_eq!(cc_types::mcp::PROTOCOL_VERSION, "2024-11-05");

    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest = fs::read_to_string(manifest_dir.join("Cargo.toml")).unwrap();
    assert!(
        !manifest.contains("cc-mcp"),
        "cc-browser must not depend on cc-mcp runtime; browser bridge uses cc-types MCP contracts"
    );
}
