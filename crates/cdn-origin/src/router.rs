use anyhow::Context;
use axum::{
    http::{header, HeaderValue},
    routing::get,
    Router,
};
use tower_http::{
    compression::CompressionLayer,
    services::ServeDir,
    set_header::SetResponseHeaderLayer,
    trace::TraceLayer,
};

use crate::config::Config;

pub fn build(cfg: &Config) -> anyhow::Result<Router> {
    let root = cfg
        .root
        .canonicalize()
        .with_context(|| format!("root {:?} does not exist", cfg.root))?;

    let cache_control = format!("public, max-age={}, stale-while-revalidate=60", cfg.max_age);
    let cache_value =
        HeaderValue::from_str(&cache_control).unwrap_or_else(|_| HeaderValue::from_static("public, max-age=3600"));

    let serve_dir = ServeDir::new(&root)
        // Serve pre-compressed .gz / .br siblings when the client accepts them
        .precompressed_gzip()
        .precompressed_br()
        // ETag + Last-Modified support is on by default in tower-http ServeDir
        .append_index_html_on_directories(true);

    let app = Router::new()
        // Health endpoint — used by edge nodes before forwarding
        .route("/health", get(health))
        .fallback_service(serve_dir)
        .layer(SetResponseHeaderLayer::if_not_present(
            header::CACHE_CONTROL,
            cache_value,
        ))
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http());

    Ok(app)
}

async fn health() -> &'static str {
    "ok"
}
