//! Build script: auto-build web-ui if dist/ is missing or stale.
//!
//! - Checks if `web-ui/dist/index.html` exists
//! - If missing or `FORCE_WEB_BUILD` is set, runs `npm run build`
//! - If npm is unavailable, prints a warning (TUI still works, --web won't)
//! - Sets cargo:rerun-if-changed so rebuilds are triggered on source changes

use std::path::Path;
use std::process::Command;

fn main() {
    let dist_index = Path::new("web-ui/dist/index.html");
    let force = std::env::var("FORCE_WEB_BUILD").is_ok();

    // Rerun if frontend source changes
    println!("cargo:rerun-if-changed=web-ui/src/");
    println!("cargo:rerun-if-changed=web-ui/package.json");
    println!("cargo:rerun-if-changed=web-ui/index.html");
    println!("cargo:rerun-if-changed=web-ui/vite.config.ts");
    println!("cargo:rerun-if-changed=web-ui/tailwind.config.js");

    if dist_index.exists() && !force {
        // dist/ is fresh, nothing to do
        return;
    }

    println!("cargo:warning=Building web-ui frontend...");

    // Check if npm is available
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

    // Install dependencies if node_modules is missing
    let node_modules = Path::new("web-ui/node_modules");
    if !node_modules.exists() {
        println!("cargo:warning=Installing web-ui dependencies...");
        let install = Command::new(npm_cmd)
            .arg("install")
            .current_dir("web-ui")
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

    // Build
    println!("cargo:warning=Running npm run build...");
    let build = Command::new(npm_cmd)
        .args(["run", "build"])
        .current_dir("web-ui")
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
