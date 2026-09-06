//! Shared HTTP client for the public APIs (Electricity Maps, price sources).

use std::sync::OnceLock;
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
