use std::net::SocketAddr;

use anyhow::{bail, Context, Result};

/// Runtime configuration, sourced entirely from the environment so the same
/// binary works locally, in Docker, and under systemd.
#[derive(Clone, Debug)]
pub struct Config {
    /// Address to bind. Keep loopback for local dev, 0.0.0.0 inside a container.
    pub bind: SocketAddr,
    /// Externally reachable base URL, used to build the returned short links.
    pub public_base_url: String,
    /// Accepted API keys. Empty means the service refuses to start.
    pub api_keys: Vec<String>,
    /// SQLite file path. `:memory:` is allowed for tests.
    pub db_path: String,
    /// Length of generated slugs.
    pub slug_len: usize,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let bind_raw = env_or("SHORTENER_BIND", "127.0.0.1:8080");
        let bind: SocketAddr = bind_raw
            .parse()
            .with_context(|| format!("SHORTENER_BIND is not a valid socket address: {bind_raw}"))?;

        let public_base_url = env_or("SHORTENER_PUBLIC_BASE_URL", "http://127.0.0.1:8080")
            .trim_end_matches('/')
            .to_string();

        let keys_raw = std::env::var("SHORTENER_API_KEYS").unwrap_or_default();
        let api_keys: Vec<String> = keys_raw
            .split(',')
            .map(str::trim)
            .filter(|k| !k.is_empty())
            .map(str::to_string)
            .collect();
        if api_keys.is_empty() {
            bail!(
                "SHORTENER_API_KEYS is empty. Set one or more comma-separated keys, e.g. \
                 SHORTENER_API_KEYS=$(openssl rand -hex 24)"
            );
        }
        if api_keys.iter().any(|k| k.len() < 16) {
            bail!("every key in SHORTENER_API_KEYS must be at least 16 characters");
        }

        let slug_len: usize = env_or("SHORTENER_SLUG_LEN", "7")
            .parse()
            .context("SHORTENER_SLUG_LEN must be an integer")?;
        if !(4..=32).contains(&slug_len) {
            bail!("SHORTENER_SLUG_LEN must be between 4 and 32");
        }

        Ok(Self {
            bind,
            public_base_url,
            api_keys,
            db_path: env_or("SHORTENER_DB", "./data/shortener.db"),
            slug_len,
        })
    }

    pub fn short_url(&self, slug: &str) -> String {
        format!("{}/{}", self.public_base_url, slug)
    }
}

fn env_or(key: &str, default: &str) -> String {
    match std::env::var(key) {
        Ok(v) if !v.trim().is_empty() => v.trim().to_string(),
        _ => default.to_string(),
    }
}
