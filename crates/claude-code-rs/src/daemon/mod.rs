//! KAIROS daemon -- HTTP server + proactive tick loop.
pub mod channels;
pub mod memory_log;
pub mod notification;
pub mod process_state;
pub mod protocol;
pub mod routes;
pub mod server;
pub mod sse;
pub mod state;
pub mod supervisor;
pub mod team_memory_proxy;
pub mod tick;
pub mod webhook;
