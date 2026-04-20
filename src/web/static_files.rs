//! Embedded static file serving for the React SPA.
//!
//! Only compiles the embedded-asset path when the `web-ui` feature is enabled
//! (which pulls in `rust-embed`). Without the feature, the fallback responds
//! with a message explaining how to build the UI or enable the feature — the
//! Axum router stays valid either way.

use axum::{
    http::{header, StatusCode, Uri},
    response::IntoResponse,
};

#[cfg(feature = "web-ui")]
use rust_embed::Embed;

#[cfg(feature = "web-ui")]
#[derive(Embed)]
#[folder = "web-ui/dist"]
struct WebAssets;

/// Fallback handler: serve embedded static files or SPA index.html.
pub async fn static_handler(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    serve_embedded_file(path)
}

#[cfg(feature = "web-ui")]
fn serve_embedded_file(path: &str) -> axum::response::Response {
    match WebAssets::get(path) {
        Some(file) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, mime.as_ref().to_string())],
                file.data.to_vec(),
            )
                .into_response()
        }
        None => {
            // SPA fallback: return index.html for all non-file routes
            match WebAssets::get("index.html") {
                Some(index) => (
                    StatusCode::OK,
                    [(header::CONTENT_TYPE, "text/html".to_string())],
                    index.data.to_vec(),
                )
                    .into_response(),
                None => (
                    StatusCode::NOT_FOUND,
                    "Web UI assets not found. Build with: cd web-ui && npm install && npm run build",
                )
                    .into_response(),
            }
        }
    }
}

#[cfg(not(feature = "web-ui"))]
fn serve_embedded_file(_path: &str) -> axum::response::Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        [(header::CONTENT_TYPE, "text/plain".to_string())],
        "Web UI assets are not bundled in this build. Rebuild with `--features web-ui` to embed the SPA.",
    )
        .into_response()
}
