//! Web server module — Axum-based HTTP server for the chat UI.

pub mod handlers;
pub mod sse;
pub mod state;
pub mod static_files;

use std::net::SocketAddr;

use axum::{
    routing::{get, post},
    Router,
};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::info;

use crate::web::state::WebState;

/// Build the Axum router with all routes.
pub fn build_router(state: WebState) -> Router {
    Router::new()
        // API routes
        .route("/api/chat", post(handlers::chat_handler))
        .route("/api/abort", post(handlers::abort_handler))
        .route("/api/state", get(handlers::state_handler))
        // Static files (SPA)
        .fallback(static_files::static_handler)
        // Middleware
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state)
}

/// Start the web server on the given port.
pub async fn start_server(state: WebState, port: u16, no_open: bool) -> anyhow::Result<()> {
    let app = build_router(state);
    let addr = SocketAddr::from(([127, 0, 0, 1], port));

    info!("Web UI starting on http://{}", addr);

    if !no_open {
        // TODO: add `open` crate to Cargo.toml to auto-open browser
        // let url = format!("http://{}", addr);
        // let _ = open::that(&url);
        info!("Open http://{} in your browser", addr);
    }

    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!("Web UI listening on http://{}", addr);
    axum::serve(listener, app).await?;

    Ok(())
}
