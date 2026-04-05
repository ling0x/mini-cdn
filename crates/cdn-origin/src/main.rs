use anyhow::Context;
use axum::http::{header, HeaderValue};
use axum::Router;
use clap::Parser;
use std::path::PathBuf;
use tokio::net::TcpListener;
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::TraceLayer;

#[derive(Parser, Debug)]
#[command(name = "cdn-origin")]
struct Args {
    /// Listen address (e.g. 127.0.0.1:4000)
    #[arg(long, default_value = "127.0.0.1:4000", env = "CDN_ORIGIN_BIND")]
    bind: String,

    /// Directory to serve as static assets
    #[arg(
        long,
        default_value = concat!(env!("CARGO_MANIFEST_DIR"), "/static"),
        env = "CDN_ORIGIN_ROOT"
    )]
    root: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "cdn_origin=info,tower_http=info".into()),
        )
        .init();

    let args = Args::parse();
    let root = args.root.canonicalize().with_context(|| {
        format!(
            "root {:?} does not exist; create it or pass --root",
            args.root
        )
    })?;

    let cache = SetResponseHeaderLayer::if_not_present(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=3600"),
    );

    let app = Router::new()
        .fallback_service(ServeDir::new(&root))
        .layer(cache)
        .layer(TraceLayer::new_for_http());

    let listener = TcpListener::bind(&args.bind)
        .await
        .with_context(|| format!("bind {}", args.bind))?;

    tracing::info!(%args.bind, root = %root.display(), "cdn-origin listening");
    axum::serve(listener, app).await.context("server error")?;
    Ok(())
}
