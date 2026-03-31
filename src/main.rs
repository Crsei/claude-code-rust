// Phase 0-8: Local/offline modules
mod types;
mod query;
mod engine;
mod tools;
mod permissions;
mod config;
mod compact;
mod utils;
mod session;
mod commands;
mod ui;

// Phase 9-13: Network modules (low priority)
mod api;
mod auth;
mod mcp;
mod analytics;
mod remote;

fn main() {
    println!("claude-code-rs v0.1.0");
}
