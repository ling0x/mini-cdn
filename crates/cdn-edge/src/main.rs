mod cache;
mod config;
mod proxy;
mod router;
mod shutdown;

use anyhow::Context;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

use crate::{
    cache::EdgeCache,
    config::Config,
    router::build,
};

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub cache:  Arc<EdgeCache>,
    pub client: reqwest::Client,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let config = Arc::new(Config::parse_args());

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(config.upstream_timeout_secs))
        .pool_max_idle_per_host(config.pool_max_idle)
        .tcp_keepalive(std::time::Duration::from_secs(90))
        .build()
        .context("build reqwest client")?;

    let state = AppState {
        cache:  Arc::new(EdgeCache::new(config.cache_max_items, config.cache_ttl_secs)),
        config: config.clone(),
        client,
    };

    let app = build(state);

    let listener = TcpListener::bind(&config.bind)
        .await
        .with_context(|| format!("bind {}", config.bind))?;

    tracing::info!(bind = %config.bind, origin = %config.origin, "cdn-edge listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown::signal())
        .await
        .context("server error")
}
