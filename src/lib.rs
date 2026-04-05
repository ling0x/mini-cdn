//! Minimal CDN-style reverse proxy: cache GET responses from an origin and serve them at the edge.

pub mod cache;
pub mod config;
pub mod handlers;
pub mod origin;
pub mod server;

pub use config::Config;

use std::net::SocketAddr;
use tracing::info;

pub async fn run(config: Config) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let addr: SocketAddr = config.bind.parse()?;
    let app = server::router(config);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!(%addr, "cdn listening");
    axum::serve(listener, app).await?;
    Ok(())
}
