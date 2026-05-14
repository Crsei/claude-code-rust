use std::fs;
use std::path::{Path, PathBuf};

fn rust_sources_under(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(path) = stack.pop() {
        for entry in fs::read_dir(&path).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
                out.push(path);
            }
        }
    }
    out
}

#[test]
fn mcp_crate_does_not_depend_on_browser_runtime() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest = fs::read_to_string(manifest_dir.join("Cargo.toml")).unwrap();
    assert!(
        !manifest.contains("cc-browser"),
        "cc-mcp must not depend on cc-browser; browser detection/rendering belongs to cc-browser"
    );

    for source in rust_sources_under(&manifest_dir.join("src")) {
        let content = fs::read_to_string(&source).unwrap();
        assert!(
            !content.contains("cc_browser"),
            "{} imports cc_browser; keep browser-specific runtime outside cc-mcp",
            source.display()
        );
    }
}

#[test]
fn mcp_server_config_keeps_browser_flag_as_data_only() {
    let config: cc_mcp::McpServerConfig = serde_json::from_value(serde_json::json!({
        "type": "stdio",
        "command": "browser-mcp",
        "browserMcp": true
    }))
    .unwrap();

    assert_eq!(config.transport, "stdio");
    assert_eq!(config.command.as_deref(), Some("browser-mcp"));
    assert_eq!(config.browser_mcp, Some(true));
}
