//! Build script: auto-build web-ui if dist/ is missing or stale.
//!
//! - Checks if `<workspace-root>/web-ui/dist/index.html` exists
//! - If missing or `FORCE_WEB_BUILD` is set, runs `npm run build`
//! - If npm is unavailable, prints a warning (TUI still works, --web won't)
//! - Sets cargo:rerun-if-changed so rebuilds are triggered on source changes
//!
//! Paths are resolved relative to the workspace root, which lives two levels
//! above this crate's manifest directory (`<workspace>/crates/claude-code-rs`).

use std::path::{Path, PathBuf};
use std::process::Command;

fn workspace_root() -> PathBuf {
    // CARGO_MANIFEST_DIR = <workspace>/crates/claude-code-rs
    let manifest_dir =
        std::env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR must be set by cargo");
    let manifest_dir = PathBuf::from(manifest_dir);
    manifest_dir
        .parent() // <workspace>/crates
        .and_then(|p| p.parent()) // <workspace>
        .map(Path::to_path_buf)
        .expect("CARGO_MANIFEST_DIR is not nested under <workspace>/crates/")
}

fn main() {
    let root = workspace_root();
    let web_ui = root.join("web-ui");
    let dist_index = web_ui.join("dist").join("index.html");
    let force = std::env::var("FORCE_WEB_BUILD").is_ok();

    // Rerun if frontend source changes.
    for rel in [
        "web-ui/src/",
        "web-ui/package.json",
        "web-ui/index.html",
        "web-ui/vite.config.ts",
        "web-ui/tailwind.config.js",
    ] {
        println!("cargo:rerun-if-changed={}", root.join(rel).display());
    }

    if dist_index.exists() && !force {
        return;
    }

    println!("cargo:warning=Building web-ui frontend...");

    let npm_cmd = if cfg!(target_os = "windows") {
        "npm.cmd"
    } else {
        "npm"
    };

    let npm_check = Command::new(npm_cmd).arg("--version").output();
    if npm_check.is_err() || !npm_check.unwrap().status.success() {
        println!("cargo:warning=npm not found — skipping web-ui build.");
        println!("cargo:warning=Web UI (--web) will show 'assets not found'.");
        println!("cargo:warning=TUI mode is unaffected.");
        println!("cargo:warning=To build web-ui: cd web-ui && npm install && npm run build");
        return;
    }

    let node_modules = web_ui.join("node_modules");
    if !node_modules.exists() {
        println!("cargo:warning=Installing web-ui dependencies...");
        let install = Command::new(npm_cmd)
            .arg("install")
            .current_dir(&web_ui)
            .status();
        match install {
            Ok(s) if s.success() => {}
            Ok(s) => {
                println!(
                    "cargo:warning=npm install failed (exit {}), skipping web build",
                    s
                );
                return;
            }
            Err(e) => {
                println!("cargo:warning=npm install error: {}, skipping web build", e);
                return;
            }
        }
    }

    println!("cargo:warning=Running npm run build...");
    let build = Command::new(npm_cmd)
        .args(["run", "build"])
        .current_dir(&web_ui)
        .status();

    match build {
        Ok(s) if s.success() => {
            println!("cargo:warning=Web UI built successfully.");
        }
        Ok(s) => {
            println!("cargo:warning=npm run build failed (exit {})", s);
            println!("cargo:warning=Web UI may not work. TUI is unaffected.");
        }
        Err(e) => {
            println!("cargo:warning=Failed to run npm: {}", e);
        }
    }
}
