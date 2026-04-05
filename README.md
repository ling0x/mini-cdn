# mini-cdn

A minimal CDN written in Rust — two binaries, one workspace.

```
crates/
├── cdn-origin/   # Static-asset origin server (source of truth)
│   ├── src/
│   │   ├── main.rs       # Entry point — wires config, router, shutdown
│   │   ├── config.rs     # Clap config (bind, root, max_age)
│   │   ├── router.rs     # Axum router — ServeDir, compression, /health
│   │   └── shutdown.rs   # Graceful SIGINT / SIGTERM handler
│   └── static/           # Default asset root
│
└── cdn-edge/     # Pull-through caching edge node
    └── src/
        ├── main.rs       # Entry point — wires config, cache, reqwest client
        ├── config.rs     # Clap config (bind, origin, cache TTL, pool, region)
        ├── cache.rs      # TTL-based DashMap cache with stale-eviction
        ├── proxy.rs      # Proxy logic — HIT/MISS, ETag/304, should_cache
        ├── router.rs     # Axum router — /health, DELETE /cache/:key, fallback
        └── shutdown.rs   # Graceful SIGINT / SIGTERM handler
```

## Quick start

```bash
# Terminal 1 — origin
cargo run -p cdn-origin

# Terminal 2 — edge
cargo run -p cdn-edge

# Fetch via edge (cache MISS on first hit, HIT on second)
curl -i http://127.0.0.1:5000/
```

## Response headers

| Header | Value |
|---|---|
| `x-cdn-cache` | `HIT` or `MISS` |
| `x-cdn-region` | e.g. `eu-west` (set via `CDN_REGION`) |
| `etag` | SHA256-derived or origin-provided |
| `cache-control` | `public, max-age=31536000, immutable` on cached hits |

## Configuration

### cdn-origin

| Env var | Default | Description |
|---|---|---|
| `CDN_ORIGIN_BIND` | `127.0.0.1:4000` | Listen address |
| `CDN_ORIGIN_ROOT` | `crates/cdn-origin/static` | Asset directory |
| `CDN_ORIGIN_MAX_AGE` | `3600` | Cache-Control max-age (seconds) |

### cdn-edge

| Env var | Default | Description |
|---|---|---|
| `CDN_EDGE_BIND` | `127.0.0.1:5000` | Listen address |
| `CDN_ORIGIN_URL` | `http://127.0.0.1:4000` | Origin base URL |
| `CDN_CACHE_MAX_ITEMS` | `2048` | Max cache entries |
| `CDN_CACHE_TTL_SECS` | `3600` | Cache TTL (seconds) |
| `CDN_UPSTREAM_TIMEOUT_SECS` | `30` | Upstream request timeout |
| `CDN_POOL_MAX_IDLE` | `64` | Idle connections per host |
| `CDN_REGION` | `local` | Region label for `x-cdn-region` |

## Cache invalidation

```bash
# Evict a key from a specific edge node
curl -X DELETE http://127.0.0.1:5000/cache/index.html
```
