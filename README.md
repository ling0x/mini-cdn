# cdn

Minimal Rust workspace that models a tiny CDN split into an **origin** (static files + cache headers) and an **edge** (HTTP reverse proxy to that origin). Both binaries are small Axum servers intended as a starting point, not a production CDN.

## Layout

| Crate | Role |
|-------|------|
| [`crates/cdn-origin`](crates/cdn-origin) | Serves a directory with `tower-http`’s `ServeDir` and adds `Cache-Control: public, max-age=3600` when the response does not already set `Cache-Control`. |
| [`crates/cdn-edge`](crates/cdn-edge) | Forwards any request to the configured origin URL, preserving path and query. Response bodies are streamed from the upstream. |

Default static content lives in [`crates/cdn-origin/static`](crates/cdn-origin/static).

## Requirements

- A recent stable Rust toolchain (2021 edition).

## Build

```bash
cargo build --workspace
```

## Run locally

Start the origin first, then the edge.

```bash
# Terminal 1 — listens on 127.0.0.1:4000 by default
cargo run -p cdn-origin

# Terminal 2 — listens on 127.0.0.1:5000, proxies to http://127.0.0.1:4000
cargo run -p cdn-edge
```

Then open `http://127.0.0.1:5000/` in a browser or use `curl`. The edge relays to the origin; you should see `Cache-Control` on successful static responses.

## Configuration

### `cdn-origin`

| Flag | Environment variable | Default |
|------|----------------------|---------|
| `--bind` | `CDN_ORIGIN_BIND` | `127.0.0.1:4000` |
| `--root` | `CDN_ORIGIN_ROOT` | `crates/cdn-origin/static` (resolved at compile time via `CARGO_MANIFEST_DIR`) |

The static root must exist; the path is canonicalized on startup.

Logging respects `RUST_LOG` (default filter: `cdn_origin=info,tower_http=info`).

### `cdn-edge`

| Flag | Environment variable | Default |
|------|----------------------|---------|
| `--bind` | `CDN_EDGE_BIND` | `127.0.0.1:5000` |
| `--origin` | `CDN_ORIGIN_URL` | `http://127.0.0.1:4000` |

The origin value is a base URL. The client’s path and query string are appended to it (for example, a request to `/foo?bar=1` is forwarded to `{origin}/foo?bar=1`). Hop-by-hop headers and `Host` are not copied verbatim to the upstream request.

Logging respects `RUST_LOG` (default: `info`).

## License

Workspace metadata declares `MIT OR Apache-2.0`; see each crate’s `Cargo.toml` for `license.workspace = true`.
