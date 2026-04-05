use anyhow::Context;
use axum::body::Body;
use axum::extract::State;
use axum::http::{Request, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::any;
use axum::Router;
use clap::Parser;
use futures_util::StreamExt;
use http_body_util::BodyExt;
use reqwest::Url;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(name = "cdn-edge")]
struct Args {
    /// Listen address (e.g. 127.0.0.1:5000)
    #[arg(long, default_value = "127.0.0.1:5000", env = "CDN_EDGE_BIND")]
    bind: String,

    /// Upstream origin base URL (no trailing path; path/query from the client request are appended)
    #[arg(long, env = "CDN_ORIGIN_URL", default_value = "http://127.0.0.1:4000")]
    origin: String,
}

#[derive(Clone)]
struct AppState {
    origin: Url,
    client: reqwest::Client,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let args = Args::parse();
    let origin: Url = args
        .origin
        .parse()
        .context("invalid --origin / CDN_ORIGIN_URL")?;

    let state = Arc::new(AppState {
        origin,
        client: reqwest::Client::builder()
            .build()
            .context("reqwest client")?,
    });

    let app = Router::new()
        .fallback(any(proxy))
        .with_state(state);

    let listener = TcpListener::bind(&args.bind)
        .await
        .with_context(|| format!("bind {}", args.bind))?;

    tracing::info!(%args.bind, origin = %args.origin, "cdn-edge listening");
    axum::serve(listener, app)
        .await
        .context("server error")?;
    Ok(())
}

async fn proxy(State(state): State<Arc<AppState>>, req: Request<Body>) -> impl IntoResponse {
    match proxy_inner(state, req).await {
        Ok(res) => res,
        Err(e) => e.into_response(),
    }
}

async fn proxy_inner(state: Arc<AppState>, req: Request<Body>) -> Result<Response, ProxyError> {
    let (parts, body) = req.into_parts();
    let method = parts.method.clone();
    let uri = parts.uri.clone();
    let headers = parts.headers;

    let url = upstream_url(&state.origin, &uri).map_err(|e| ProxyError::BadGateway(e.to_string()))?;

    let bytes = body
        .collect()
        .await
        .map_err(|e| ProxyError::BadRequest(e.to_string()))?
        .to_bytes();

    let mut rb = state.client.request(method, url.as_str());
    for (key, value) in headers.iter() {
        let name = key.as_str();
        if is_hop_by_hop(name) || name.eq_ignore_ascii_case("host") {
            continue;
        }
        rb = rb.header(key, value);
    }

    let upstream = rb
        .body(bytes)
        .send()
        .await
        .map_err(|e| ProxyError::BadGateway(e.to_string()))?;

    let status = upstream.status();
    let mut res = Response::builder().status(
        StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY),
    );

    for (key, value) in upstream.headers().iter() {
        if is_hop_by_hop(key.as_str()) {
            continue;
        }
        res = res.header(key, value);
    }

    let stream = upstream.bytes_stream().map(|chunk| {
        chunk.map_err(|e| std::io::Error::other(e.to_string()))
    });

    res.body(Body::from_stream(stream))
        .map_err(|e| ProxyError::BadGateway(e.to_string()))
}

fn upstream_url(origin: &Url, uri: &Uri) -> anyhow::Result<Url> {
    let pq = uri.path_and_query().map(|pq| pq.as_str()).unwrap_or("/");
    let base = origin.as_str().trim_end_matches('/');
    Ok(Url::parse(&format!("{base}{pq}"))?)
}

fn is_hop_by_hop(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "connection"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
    )
}

enum ProxyError {
    BadRequest(String),
    BadGateway(String),
}

impl IntoResponse for ProxyError {
    fn into_response(self) -> Response {
        match self {
            ProxyError::BadRequest(msg) => {
                tracing::warn!(%msg, "bad request");
                (StatusCode::BAD_REQUEST, msg).into_response()
            }
            ProxyError::BadGateway(msg) => {
                tracing::warn!(%msg, "bad gateway");
                (StatusCode::BAD_GATEWAY, msg).into_response()
            }
        }
    }
}
