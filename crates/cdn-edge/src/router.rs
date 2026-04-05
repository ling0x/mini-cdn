use axum::{
    Router,
    extract::State,
    http::Request,
    body::Body,
    routing::{any, get, delete},
    extract::Path,
    response::IntoResponse,
    http::StatusCode,
};
use std::sync::Arc;
use tower_http::trace::TraceLayer;

use crate::{AppState, proxy};

pub fn build(state: AppState) -> Router {
    let state = Arc::new(state);
    Router::new()
        .route("/health",          get(health))
        .route("/cache/:key",      delete(invalidate))
        .fallback(any(forward))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn health() -> &'static str {
    "ok"
}

/// Called by the origin (or ops tooling) to evict a specific key from this edge.
async fn invalidate(
    State(state): State<Arc<AppState>>,
    Path(key): Path<String>,
) -> impl IntoResponse {
    state.cache.invalidate(&key);
    tracing::info!(%key, "cache entry invalidated");
    StatusCode::NO_CONTENT
}

async fn forward(
    State(state): State<Arc<AppState>>,
    req: Request<Body>,
) -> impl IntoResponse {
    proxy::handle(state, req).await
}
