mod config;
mod router;
mod shutdown;

use anyhow::Context;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "cdn_origin=info,tower_http=info".into()),
        )
        .init();

    let cfg = config::Config::parse_args();
    let app = router::build(&cfg)?;

    let listener = TcpListener::bind(&cfg.bind)
        .await
        .with_context(|| format!("bind {}", cfg.bind))?;

    tracing::info!(bind = %cfg.bind, root = %cfg.root.display(), "cdn-origin listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown::signal())
        .await
        .context("server error")
}
