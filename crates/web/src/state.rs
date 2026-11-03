//! State shared by every handler.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use rackscreen_app::hub::Hub;
use rackscreen_sources::spotify::Login;

use crate::update::UpdateState;

/// How long a session cookie is good for.
pub const SESSION_TTL: Duration = Duration::from_secs(30 * 24 * 3600);
/// How long a started Spotify login (its PKCE verifier) is kept.
pub const SPOTIFY_LOGIN_TTL: Duration = Duration::from_secs(600);

/// A PNG keyed by the frame sequence number it encodes.
pub type CachedPng = Option<(u64, Arc<Vec<u8>>)>;

pub struct PendingLogin {
    pub client_id: String,
    pub login: Login,
    pub created: Instant,
}

pub struct AppState {
    pub hub: Arc<Hub>,
    /// Session id (hex) to expiry.
    sessions: Mutex<HashMap<String, Instant>>,
    /// The current `web.password`; empty means no login.
    password: RwLock<String>,
    pub spotify: Mutex<Option<PendingLogin>>,
    pub update: Mutex<UpdateState>,
    /// PNG of the latest encoded frame per screen, keyed by its sequence
    /// number, so several live clients share one encode.
    pub png_cache: Mutex<Vec<CachedPng>>,
    /// Config writes one at a time.
    pub config_lock: tokio::sync::Mutex<()>,
    /// The port the server listens on (for the address list).
    pub port: u16,
}

impl AppState {
    pub fn new(hub: Arc<Hub>, password: String, port: u16) -> AppState {
        let screens = hub.frames.screens();
        AppState {
            hub,
            sessions: Mutex::new(HashMap::new()),
            password: RwLock::new(password),
            spotify: Mutex::new(None),
            update: Mutex::new(UpdateState::default()),
            png_cache: Mutex::new(vec![None; screens]),
            config_lock: tokio::sync::Mutex::new(()),
            port,
        }
    }

    pub fn password(&self) -> String {
        self.password.read().unwrap().clone()
    }

    pub fn set_password(&self, p: &str) {
        let mut cur = self.password.write().unwrap();
        if *cur != p {
            *cur = p.to_string();
            // every session was made under the old password
            self.sessions.lock().unwrap().clear();
        }
    }

    pub fn login_required(&self) -> bool {
        !self.password.read().unwrap().is_empty()
    }

    /// A new session id, remembered until `SESSION_TTL` from now.
    pub fn new_session(&self) -> String {
        let mut bytes = [0u8; 32];
        getrandom::fill(&mut bytes).expect("os random");
        let id = hex::encode(bytes);
        let mut s = self.sessions.lock().unwrap();
        let now = Instant::now();
        s.retain(|_, exp| *exp > now);
        s.insert(id.clone(), now + SESSION_TTL);
        id
    }

    pub fn session_valid(&self, id: &str) -> bool {
        let s = self.sessions.lock().unwrap();
        s.get(id).is_some_and(|exp| *exp > Instant::now())
    }

    pub fn drop_session(&self, id: &str) {
        self.sessions.lock().unwrap().remove(id);
    }

    /// Live sessions (for tests).
    pub fn session_count_for_test(&self) -> usize {
        self.sessions.lock().unwrap().len()
    }
}
