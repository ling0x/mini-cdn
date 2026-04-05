use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug, Clone)]
#[command(name = "cdn-origin", about = "Static-asset origin server")]
pub struct Config {
    /// TCP address to listen on
    #[arg(long, default_value = "127.0.0.1:4000", env = "CDN_ORIGIN_BIND")]
    pub bind: String,

    /// Directory of static assets to serve
    #[arg(
        long,
        default_value = concat!(env!("CARGO_MANIFEST_DIR"), "/static"),
        env = "CDN_ORIGIN_ROOT"
    )]
    pub root: PathBuf,

    /// Default Cache-Control max-age in seconds (applied when origin doesn't set one)
    #[arg(long, default_value_t = 3600, env = "CDN_ORIGIN_MAX_AGE")]
    pub max_age: u32,
}

impl Config {
    pub fn parse_args() -> Self {
        Self::parse()
    }
}
