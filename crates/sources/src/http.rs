//! Shared HTTP client for the public APIs (Electricity Maps, Open-Meteo, ...),
//! and the helpers for downloads that must stay small.

use std::sync::OnceLock;

use anyhow::{bail, Context, Result};
use std::time::Duration;

pub fn client() -> reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            let _ = rustls::crypto::ring::default_provider().install_default();
            reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .user_agent(concat!("rackscreen/", env!("CARGO_PKG_VERSION")))
                .build()
                .expect("reqwest client")
        })
        .clone()
}

/// The body of `resp`, read chunk by chunk and refused as soon as it is
/// larger than `max` bytes (a `Content-Length` over `max` is refused before
/// reading). `what` names the download in the error.
pub async fn read_capped(mut resp: reqwest::Response, max: usize, what: &str) -> Result<Vec<u8>> {
    let too_big = || format!("{what} is larger than {}", fmt_size(max));
    if resp.content_length().is_some_and(|n| n > max as u64) {
        bail!(too_big());
    }
    let mut body = Vec::new();
    while let Some(chunk) = resp.chunk().await.with_context(|| format!("read {what}"))? {
        if body.len() + chunk.len() > max {
            bail!(too_big());
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

/// `2 MB`, `20 MB`, or bytes below a megabyte.
fn fmt_size(n: usize) -> String {
    if n >= 1024 * 1024 {
        format!("{} MB", n / (1024 * 1024))
    } else {
        format!("{n} bytes")
    }
}

/// Run blocking file or decode work off the async threads.
pub async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Result<T> {
    tokio::task::spawn_blocking(f)
        .await
        .context("blocking task")
}
