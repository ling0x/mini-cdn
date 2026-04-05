use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command(name = "cdn-edge", about = "CDN edge pull-through cache")]
pub struct Config {
    /// TCP address to listen on
    #[arg(long, default_value = "127.0.0.1:5000", env = "CDN_EDGE_BIND")]
    pub bind: String,

    /// Upstream origin base URL (no trailing slash)
    #[arg(long, default_value = "http://127.0.0.1:4000", env = "CDN_ORIGIN_URL")]
    pub origin: String,

    /// Maximum number of entries in the in-memory cache
    #[arg(long, default_value_t = 2048, env = "CDN_CACHE_MAX_ITEMS")]
    pub cache_max_items: usize,

    /// Time-to-live for cached entries in seconds
    #[arg(long, default_value_t = 3600, env = "CDN_CACHE_TTL_SECS")]
    pub cache_ttl_secs: u64,

    /// Upstream request timeout in seconds
    #[arg(long, default_value_t = 30, env = "CDN_UPSTREAM_TIMEOUT_SECS")]
    pub upstream_timeout_secs: u64,

    /// Max idle connections per host in the connection pool
    #[arg(long, default_value_t = 64, env = "CDN_POOL_MAX_IDLE")]
    pub pool_max_idle: usize,

    /// Region label attached to x-cdn-region header (observability)
    #[arg(long, default_value = "local", env = "CDN_REGION")]
    pub region: String,
}

impl Config {
    pub fn parse_args() -> Self {
        Self::parse()
    }
}
