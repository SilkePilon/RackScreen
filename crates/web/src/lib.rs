//! The web UI served by the running service: an axum server on its own
//! tokio runtime and thread (it outlives monitor restarts), the HTTP API the
//! frontend is built against, and the embedded frontend itself.

pub mod auth;
pub mod config_api;
pub mod control;
pub mod gif_memory;
pub mod gif_search;
pub mod host;
pub mod live;
pub mod logs;
pub mod roles;
pub mod show_cards;
pub mod spotify;
pub mod state;
pub mod static_files;
pub mod status;
pub mod system;
pub mod tailscale;
pub mod update;

use std::net::SocketAddr;
use std::sync::Arc;
use std::thread::JoinHandle;

use anyhow::{Context, Result};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use rackscreen_app::hub::Hub;

pub use state::AppState;

/// Every API error is `{"error": "..."}` with a 4xx/5xx status.
#[derive(Debug)]
pub struct ApiError(pub StatusCode, pub String);

impl ApiError {
    pub fn bad_request(msg: impl Into<String>) -> ApiError {
        ApiError(StatusCode::BAD_REQUEST, msg.into())
    }
    pub fn not_found(msg: impl Into<String>) -> ApiError {
        ApiError(StatusCode::NOT_FOUND, msg.into())
    }
    pub fn internal(msg: impl Into<String>) -> ApiError {
        ApiError(StatusCode::INTERNAL_SERVER_ERROR, msg.into())
    }
    pub fn unavailable(msg: impl Into<String>) -> ApiError {
        ApiError(StatusCode::SERVICE_UNAVAILABLE, msg.into())
    }
    pub fn conflict(msg: impl Into<String>) -> ApiError {
        ApiError(StatusCode::CONFLICT, msg.into())
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self {
        ApiError::bad_request(format!("{e:#}"))
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(serde_json::json!({ "error": self.1 }))).into_response()
    }
}

pub type ApiResult<T> = Result<Json<T>, ApiError>;

/// The whole app: the API under `/api` behind the CSRF and session guard,
/// the embedded UI everywhere else.
pub fn router(state: Arc<AppState>) -> Router {
    let api = Router::new()
        .route("/auth", get(auth::get_auth))
        .route("/auth/login", post(auth::login))
        .route("/auth/logout", post(auth::logout))
        .route("/info", get(status::info))
        .route("/status", get(status::status))
        .route("/roles", get(roles::roles))
        .route("/shows", get(roles::shows))
        .route("/shows/{id}/card.png", get(show_cards::card))
        .route(
            "/config",
            get(config_api::get_config).put(config_api::put_config),
        )
        .route(
            "/config/yaml",
            get(config_api::get_yaml).put(config_api::put_yaml),
        )
        .route("/config/validate", post(config_api::validate))
        .route("/control/identify", post(control::identify))
        .route("/control/pattern", post(control::pattern))
        .route("/control/role", post(control::role))
        .route("/control/play", post(control::play))
        .route("/control/transition-test", post(control::transition_test))
        .route("/control/night", post(control::night))
        .route("/control/restart", post(control::restart))
        .route("/system", get(system::system))
        .route("/gifs/memory", get(gif_memory::memory))
        .route("/gifs/providers", get(gif_search::providers))
        .route("/gifs/search", get(gif_search::search))
        .route("/gifs/trending", get(gif_search::trending))
        .route("/gifs/trending-terms", get(gif_search::trending_terms))
        .route("/live", get(live::live))
        .route("/logs", get(logs::logs))
        .route("/logs/stream", get(logs::stream))
        .route("/spotify", get(spotify::get_spotify))
        .route("/spotify/login", post(spotify::login))
        .route("/spotify/callback", post(spotify::callback))
        .route("/spotify/logout", post(spotify::logout))
        .route("/update", get(update::get_update).post(update::post_update))
        .route("/tailscale", get(tailscale::get_tailscale))
        .route(
            "/tailscale/setup",
            get(tailscale::get_setup).post(tailscale::post_setup),
        )
        .route("/tailscale/login", post(tailscale::login))
        .route("/tailscale/up", post(tailscale::up))
        .route("/tailscale/down", post(tailscale::down))
        .route("/tailscale/logout", post(tailscale::logout))
        .route("/tailscale/ssh", post(tailscale::ssh))
        .route("/tailscale/serve", post(tailscale::serve))
        .fallback(api_not_found)
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::guard,
        ));
    Router::new()
        .nest("/api", api)
        .fallback(static_files::serve)
        .with_state(state)
}

async fn api_not_found() -> ApiError {
    ApiError::not_found("no such endpoint")
}

/// A running server: its thread and the address it listens on.
pub struct WebServer {
    pub addr: SocketAddr,
    thread: Option<JoinHandle<()>>,
}

impl WebServer {
    pub fn join(mut self) {
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// Bind `bind:port` and serve on a new thread with its own runtime. The
/// bind happens before this returns, so a port in use is an error here.
pub fn serve(hub: Arc<Hub>, bind: &str, port: u16, password: String) -> Result<WebServer> {
    let addr: SocketAddr = format!("{bind}:{port}")
        .parse()
        .with_context(|| format!("web.bind {bind}:{port}"))?;
    let std_listener = std::net::TcpListener::bind(addr).with_context(|| format!("bind {addr}"))?;
    std_listener.set_nonblocking(true)?;
    let addr = std_listener.local_addr()?;
    let state = AppState::new(hub, password, addr.port());
    *state.tailscale.bind.lock().unwrap() = bind.to_string();
    let state = Arc::new(state);
    let thread = std::thread::Builder::new()
        .name("web".into())
        .spawn(move || {
            // One browser or two on a Pi: one worker is plenty.
            let runtime = match tokio::runtime::Builder::new_multi_thread()
                .worker_threads(1)
                .max_blocking_threads(2)
                .enable_all()
                .build()
            {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!("web runtime: {e}");
                    return;
                }
            };
            runtime.block_on(async move {
                let listener = match tokio::net::TcpListener::from_std(std_listener) {
                    Ok(l) => l,
                    Err(e) => {
                        tracing::error!("web listener: {e}");
                        return;
                    }
                };
                if let Err(e) = axum::serve(listener, router(state)).await {
                    tracing::error!("web server: {e}");
                }
            });
        })?;
    tracing::info!("web UI on http://{addr}");
    Ok(WebServer {
        addr,
        thread: Some(thread),
    })
}
