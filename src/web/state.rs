//! Shared state for the web server layer.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use crate::engine::lifecycle::QueryEngine;

/// Shared state passed to all Axum handlers via State extractor.
#[derive(Clone)]
pub struct WebState {
    /// Reference to the query engine.
    pub engine: Arc<QueryEngine>,
    /// Flag: is a query currently in progress?
    pub is_streaming: Arc<AtomicBool>,
}
