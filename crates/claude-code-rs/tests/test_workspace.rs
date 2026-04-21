/// Cross-platform test workspace directory.
///
/// Include in integration test files with:
///   #[path = "test_workspace.rs"] mod test_workspace;
///   use test_workspace::workspace;
///
/// Or copy the `workspace()` function directly.

/// Returns the test workspace path. Checks `E2E_WORKSPACE` env var first,
/// then falls back to platform default. Creates the directory if needed.
pub fn workspace() -> &'static str {
    static WS: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    WS.get_or_init(|| {
        let dir = std::env::var("E2E_WORKSPACE").unwrap_or_else(|_| {
            if cfg!(windows) {
                r"F:\temp".to_string()
            } else {
                "/tmp/cc-rust-test".to_string()
            }
        });
        std::fs::create_dir_all(&dir).ok();
        dir
    })
}
