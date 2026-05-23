//! Real-project MCP capability tests.
//!
//! The default tests exercise discovery, disable behavior, and error recovery
//! without downloading third-party packages. Ignored tests connect to pinned
//! community MCP packages with `npx` / `uvx`.

mod capability_lab_support;

use capability_lab_support::{
    context7_server_config, filesystem_server_config, git_server_config, github_server_config,
    playwright_server_config, sequential_server_config, CapabilityLab,
};
use cc_mcp::discovery::{discover_mcp_servers, discover_mcp_servers_scoped, DiscoveryScope};
use cc_mcp::manager::McpManager;
use cc_mcp::ToolCallContent;
use serde_json::json;
use serial_test::serial;

#[test]
#[serial]
fn mcp_discovery_reads_project_settings_and_respects_disabled_entries() {
    let lab = CapabilityLab::new();
    let (_home, _cc_home) = lab.set_env();
    lab.write_project_mcp_settings(json!({
        "filesystem": {
            "type": "stdio",
            "command": "npx",
            "args": ["-y", capability_lab_support::FS_SERVER_PACKAGE, lab.project_dir],
            "env": { "npm_config_cache": lab.npm_cache_dir() }
        },
        "disabled-sequential": {
            "type": "stdio",
            "command": "npx",
            "args": ["-y", capability_lab_support::SEQUENTIAL_SERVER_PACKAGE],
            "env": { "npm_config_cache": lab.npm_cache_dir() },
            "disabled": true
        }
    }));

    let scoped = discover_mcp_servers_scoped(&lab.project_dir).expect("discover scoped MCP");
    assert!(scoped.iter().any(|entry| {
        entry.scope == DiscoveryScope::Project && entry.config.name == "filesystem"
    }));
    assert!(scoped.iter().any(|entry| {
        entry.scope == DiscoveryScope::Project
            && entry.config.name == "disabled-sequential"
            && entry.config.disabled == Some(true)
    }));

    let merged = discover_mcp_servers(&lab.project_dir).expect("discover merged MCP");
    assert_eq!(merged.len(), 2);
    lab.assert_path_isolated();
}

#[tokio::test]
#[serial]
async fn mcp_manager_skips_disabled_server_without_failing_session() {
    let mut manager = McpManager::new();
    manager
        .connect_server(cc_mcp::McpServerConfig {
            name: "disabled".to_string(),
            transport: "stdio".to_string(),
            command: Some("definitely-not-a-real-mcp-command".to_string()),
            args: Some(vec![]),
            url: None,
            headers: None,
            oauth: None,
            env: None,
            browser_mcp: None,
            disabled: Some(true),
        })
        .await
        .expect("disabled server should be skipped cleanly");
    assert!(manager.clients.is_empty());
}

#[tokio::test]
#[serial]
async fn mcp_manager_recovers_from_missing_command() {
    let mut manager = McpManager::new();
    let result = manager
        .connect_server(cc_mcp::McpServerConfig {
            name: "missing-command".to_string(),
            transport: "stdio".to_string(),
            command: Some("cc-rust-missing-mcp-command".to_string()),
            args: Some(vec![]),
            url: None,
            headers: None,
            oauth: None,
            env: None,
            browser_mcp: None,
            disabled: None,
        })
        .await;
    assert!(result.is_err());
    assert!(manager.clients.is_empty());
}

#[tokio::test]
#[ignore = "downloads and runs @modelcontextprotocol/server-filesystem via npx"]
#[serial]
async fn community_filesystem_mcp_lists_tools_and_reads_project_doc() {
    let lab = CapabilityLab::new();
    let (_home, _cc_home) = lab.set_env();
    let mut manager = McpManager::new();
    manager
        .connect_server(filesystem_server_config(
            &lab.project_dir,
            &lab.npm_cache_dir(),
        ))
        .await
        .expect("connect filesystem MCP");

    let client = manager
        .clients
        .get("filesystem")
        .expect("filesystem client connected");
    assert!(client.tools.iter().any(|tool| tool.name == "read_file"));

    let result = client
        .call_tool(
            "read_file",
            json!({ "path": lab.project_dir.join("docs/product-brief.md") }),
        )
        .await
        .expect("read project doc through filesystem MCP");
    let text = tool_result_text(&result);
    assert!(text.contains("Product Brief"));

    let forbidden = client
        .call_tool("read_file", json!({ "path": "/etc/passwd" }))
        .await
        .expect("filesystem MCP should return a structured tool error");
    assert!(
        forbidden.is_error || !tool_result_text(&forbidden).contains("root:"),
        "filesystem MCP must not leak /etc/passwd contents: {:?}",
        forbidden
    );
}

fn tool_result_text(result: &cc_mcp::CallToolResult) -> String {
    result
        .content
        .iter()
        .filter_map(|content| match content {
            ToolCallContent::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[tokio::test]
#[ignore = "downloads and runs mcp-server-git via uvx"]
#[serial]
async fn community_git_mcp_lists_repository_tools() {
    let lab = CapabilityLab::new();
    let (_home, _cc_home) = lab.set_env();
    lab.init_git();
    let mut manager = McpManager::new();
    manager
        .connect_server(git_server_config(&lab.project_dir))
        .await
        .expect("connect git MCP");

    let client = manager.clients.get("git").expect("git client connected");
    assert!(
        client
            .tools
            .iter()
            .any(|tool| tool.name.contains("status") || tool.name.contains("diff")),
        "expected git status/diff-like tools, got {:?}",
        client.tools
    );
}

#[tokio::test]
#[ignore = "downloads and runs @modelcontextprotocol/server-sequential-thinking via npx"]
#[serial]
async fn community_sequential_thinking_mcp_executes_plan_step() {
    let lab = CapabilityLab::new();
    let (_home, _cc_home) = lab.set_env();
    let mut manager = McpManager::new();
    manager
        .connect_server(sequential_server_config(&lab.npm_cache_dir()))
        .await
        .expect("connect sequential-thinking MCP");

    let client = manager
        .clients
        .get("sequential-thinking")
        .expect("sequential client connected");
    let tool = client
        .tools
        .iter()
        .find(|tool| tool.name.contains("sequential"))
        .expect("sequential thinking tool");
    let result = client
        .call_tool(
            &tool.name,
            json!({
                "thought": "Plan a five-step fix for a failing issue status UI test.",
                "thoughtNumber": 1,
                "totalThoughts": 5,
                "nextThoughtNeeded": false
            }),
        )
        .await
        .expect("call sequential thinking tool");
    assert!(!result.content.is_empty());
}

#[tokio::test]
#[ignore = "downloads and runs @playwright/mcp via npx and requires browser dependencies"]
#[serial]
async fn community_playwright_mcp_connects_and_lists_browser_tools() {
    let lab = CapabilityLab::new();
    let (_home, _cc_home) = lab.set_env();
    let mut manager = McpManager::new();
    manager
        .connect_server(playwright_server_config(&lab.npm_cache_dir()))
        .await
        .expect("connect Playwright MCP");
    let client = manager
        .clients
        .get("playwright")
        .expect("playwright client connected");
    assert!(
        client.tools.iter().any(|tool| {
            let name = tool.name.to_ascii_lowercase();
            name.contains("browser") || name.contains("navigate") || name.contains("page")
        }),
        "expected browser-oriented Playwright MCP tools, got {:?}",
        client.tools
    );
}

#[tokio::test]
#[ignore = "downloads and runs @upstash/context7-mcp via npx and requires network for docs calls"]
#[serial]
async fn community_context7_mcp_lists_documentation_tools() {
    let lab = CapabilityLab::new();
    let (_home, _cc_home) = lab.set_env();
    let mut manager = McpManager::new();
    manager
        .connect_server(context7_server_config(&lab.npm_cache_dir()))
        .await
        .expect("connect Context7 MCP");
    let client = manager.clients.get("context7").expect("context7 client");
    assert!(
        client.tools.iter().any(|tool| {
            let name = tool.name.to_ascii_lowercase();
            name.contains("resolve") || name.contains("docs")
        }),
        "expected Context7 documentation tools, got {:?}",
        client.tools
    );
}

#[tokio::test]
#[ignore = "downloads deprecated @modelcontextprotocol/server-github via npx; success path requires GITHUB_TOKEN"]
#[serial]
async fn community_github_mcp_lists_tools_or_reports_missing_token() {
    let lab = CapabilityLab::new();
    let (_home, _cc_home) = lab.set_env();
    let mut manager = McpManager::new();
    let result = manager
        .connect_server(github_server_config(&lab.npm_cache_dir()))
        .await;

    if std::env::var("GITHUB_TOKEN").is_ok() {
        result.expect("connect GitHub MCP with token");
        let client = manager.clients.get("github").expect("github client");
        assert!(
            client
                .tools
                .iter()
                .any(|tool| tool.name.contains("repository") || tool.name.contains("issue")),
            "expected GitHub repository/issue tools, got {:?}",
            client.tools
        );
    } else {
        assert!(
            result.is_err() || manager.clients.contains_key("github"),
            "GitHub MCP without token should either surface a startup error or connect with tool-level auth errors"
        );
    }
}
