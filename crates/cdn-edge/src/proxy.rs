use axum::{
    body::Body,
    http::{Request, Response, StatusCode, Uri, header},
    response::IntoResponse,
};
use bytes::Bytes;
use futures_util::StreamExt;
use http_body_util::BodyExt;
use reqwest::Url;
use std::{sync::Arc, time::Instant};

use crate::{AppState, cache::CachedResponse};

// ── Error type ────────────────────────────────────────────────────────────────

pub enum ProxyError {
    BadRequest(String),
    BadGateway(String),
    NotFound,
}

impl IntoResponse for ProxyError {
    fn into_response(self) -> Response<Body> {
        match self {
            ProxyError::BadRequest(m) => {
                tracing::warn!(%m, "bad request");
                (StatusCode::BAD_REQUEST, m).into_response()
            }
            ProxyError::BadGateway(m) => {
                tracing::warn!(%m, "bad gateway");
                (StatusCode::BAD_GATEWAY, m).into_response()
            }
            ProxyError::NotFound => StatusCode::NOT_FOUND.into_response(),
        }
    }
}

// ── Public handler ────────────────────────────────────────────────────────────

pub async fn handle(
    state: Arc<AppState>,
    req: Request<Body>,
) -> Response<Body> {
    match proxy_inner(state, req).await {
        Ok(r)  => r,
        Err(e) => e.into_response(),
    }
}

// ── Core logic ────────────────────────────────────────────────────────────────

async fn proxy_inner(
    state: Arc<AppState>,
    req: Request<Body>,
) -> Result<Response<Body>, ProxyError> {
    let (parts, body) = req.into_parts();
    let cache_key = parts.uri.path().to_string();
    let is_get = parts.method == axum::http::Method::GET;

    // ── 1. Cache HIT (GET only) ───────────────────────────────────────────────
    if is_get {
        if let Some(cached) = state.cache.get(&cache_key) {
            // Conditional GET: ETag match → 304
            if let Some(ref etag) = cached.etag {
                if parts.headers.get(header::IF_NONE_MATCH)
                    .and_then(|v| v.to_str().ok())
                    .map(|v| v == etag)
                    .unwrap_or(false)
                {
                    return Ok(not_modified_response(etag, &state.config.region));
                }
            }
            tracing::debug!(key = %cache_key, "cache HIT");
            return Ok(cached_response(cached, &state.config.region));
        }
    }

    // ── 2. Cache MISS — build upstream request ────────────────────────────────
    let url = upstream_url(&state.config.origin, &parts.uri)
        .map_err(|e| ProxyError::BadGateway(e.to_string()))?;

    // Pipe body as a stream instead of buffering
    let bytes: Bytes = body
        .collect()
        .await
        .map_err(|e| ProxyError::BadRequest(e.to_string()))?
        .to_bytes();

    let mut rb = state.client.request(parts.method.clone(), url.as_str());

    for (k, v) in parts.headers.iter() {
        let name = k.as_str();
        if is_hop_by_hop(name) || name.eq_ignore_ascii_case("host") {
            continue;
        }
        rb = rb.header(k, v);
    }

    // Identify this edge to upstream
    rb = rb.header("via", "1.1 cdn-edge");

    let upstream = rb
        .body(bytes)
        .send()
        .await
        .map_err(|e| ProxyError::BadGateway(e.to_string()))?;

    let status = upstream.status();

    // ── 3. Optionally cache the upstream response ─────────────────────────────
    let content_type = upstream
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_string();

    let etag = upstream
        .headers()
        .get(reqwest::header::ETAG)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let cacheable = is_get
        && status.is_success()
        && should_cache(upstream.headers());

    let resp_bytes = Bytes::from(
        upstream
            .bytes()
            .await
            .map_err(|e| ProxyError::BadGateway(e.to_string()))?,
    );

    if cacheable {
        tracing::debug!(key = %cache_key, "cache MISS → storing");
        state.cache.insert(
            cache_key,
            CachedResponse {
                body: resp_bytes.clone(),
                content_type: content_type.clone(),
                etag: etag.clone(),
                expires_at: Instant::now() + state.cache.ttl(),
            },
        );
    }

    // ── 4. Build response to client ───────────────────────────────────────────
    let http_status = StatusCode::from_u16(status.as_u16())
        .unwrap_or(StatusCode::BAD_GATEWAY);

    let mut builder = Response::builder().status(http_status);
    builder = builder
        .header(header::CONTENT_TYPE, &content_type)
        .header("x-cdn-cache", "MISS")
        .header("x-cdn-region", &state.config.region);

    if let Some(ref e) = etag {
        builder = builder.header(header::ETAG, e);
    }

    builder
        .body(Body::from(resp_bytes))
        .map_err(|e| ProxyError::BadGateway(e.to_string()))
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn upstream_url(origin: &str, uri: &Uri) -> anyhow::Result<Url> {
    let pq  = uri.path_and_query().map(|p| p.as_str()).unwrap_or("/");
    let base = origin.trim_end_matches('/');
    Ok(Url::parse(&format!("{base}{pq}"))?)
}

fn is_hop_by_hop(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "connection" | "keep-alive" | "proxy-authenticate"
            | "proxy-authorization" | "te" | "trailer"
            | "transfer-encoding" | "upgrade"
    )
}

/// Returns false when the origin signals the response must not be cached.
fn should_cache(headers: &reqwest::header::HeaderMap) -> bool {
    if let Some(cc) = headers
        .get(reqwest::header::CACHE_CONTROL)
        .and_then(|v| v.to_str().ok())
    {
        // Honour no-store and private directives
        if cc.contains("no-store") || cc.contains("private") {
            return false;
        }
    }
    true
}

fn cached_response(entry: CachedResponse, region: &str) -> Response<Body> {
    let mut b = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, &entry.content_type)
        .header("x-cdn-cache", "HIT")
        .header("x-cdn-region", region)
        .header(header::CACHE_CONTROL, "public, max-age=31536000, immutable");

    if let Some(ref e) = entry.etag {
        b = b.header(header::ETAG, e);
    }

    b.body(Body::from(entry.body)).unwrap()
}

fn not_modified_response(etag: &str, region: &str) -> Response<Body> {
    Response::builder()
        .status(StatusCode::NOT_MODIFIED)
        .header(header::ETAG, etag)
        .header("x-cdn-cache", "HIT")
        .header("x-cdn-region", region)
        .body(Body::empty())
        .unwrap()
}
