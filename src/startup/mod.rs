//! Startup helpers — env loading, tracing setup, fast-path handlers,
//! runtime-config assembly, and non-interactive output modes.
//!
//! Entry point is [`main.rs`]; this module owns the pieces main would
//! otherwise inline. Split out of the monolithic `main.rs` per issue #22
//! to keep the entry point focused on orchestration.

pub mod fast_paths;
pub mod logging;
pub mod modes;
pub mod runtime_config;

use crate::config::settings;

/// Load `.env` files in priority order (later loads do NOT override earlier):
///   1. `~/.cc-rust/.env`        (global user config)
///   2. `<exe-dir>/.env`         (portable, next to the binary)
///   3. `<cwd>/.env`             (project-local)
pub fn load_env_files() {
    if let Ok(global_dir) = settings::global_claude_dir() {
        let global_env = global_dir.join(".env");
        let _ = dotenvy::from_path(&global_env);
    }
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let exe_env = exe_dir.join(".env");
            let _ = dotenvy::from_path(&exe_env);
        }
    }
    let _ = dotenvy::dotenv();
}
