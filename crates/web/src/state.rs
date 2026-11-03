//! State shared by every handler.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use rackscreen_app::hub::Hub;
use rackscreen_sources::spotify::Login;

use crate::update::UpdateState;

/// How long a session cookie is good for.
pub const SESSION_TTL: Duration = Duration::from_secs(30 * 24 * 3600);
/// Failed logins allowed per `LOGIN_WINDOW` (for everyone together) before
/// the login answers 429.
pub const LOGIN_MAX_FAILURES: usize = 5;
pub const LOGIN_WINDOW: Duration = Duration::from_secs(60);
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
    /// When the recent failed logins happened (at most `LOGIN_MAX_FAILURES`).
    login_failures: Mutex<VecDeque<Instant>>,
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
        let update = UpdateState::load(crate::update::job_path(), hub.version);
        AppState {
            hub,
            sessions: Mutex::new(HashMap::new()),
            login_failures: Mutex::new(VecDeque::new()),
            password: RwLock::new(password),
            spotify: Mutex::new(None),
            update: Mutex::new(update),
            png_cache: Mutex::new(vec![None; screens]),
            config_lock: tokio::sync::Mutex::new(()),
            port,
        }
    }

    /// Forget the encoded PNGs of screens at or past `active`.
    pub fn trim_png_cache(&self, active: usize) {
        let mut cache = self.png_cache.lock().unwrap();
        for slot in cache.iter_mut().skip(active) {
            *slot = None;
        }
    }

    pub fn password(&self) -> String {
        self.password.read().unwrap().clone()
    }

    /// Change the password. Every session was made under the old one and
    /// ends, except `keep` (the session that made the change).
    pub fn set_password(&self, p: &str, keep: Option<&str>) {
        let mut cur = self.password.write().unwrap();
        if *cur != p {
            *cur = p.to_string();
            let mut s = self.sessions.lock().unwrap();
            let now = Instant::now();
            s.retain(|id, exp| *exp > now && keep == Some(id.as_str()));
        }
    }

    /// `Some(wait)` while too many logins failed in the last `LOGIN_WINDOW`.
    pub fn login_blocked(&self) -> Option<Duration> {
        self.login_blocked_at(Instant::now())
    }

    fn login_blocked_at(&self, now: Instant) -> Option<Duration> {
        let mut f = self.login_failures.lock().unwrap();
        while f
            .front()
            .is_some_and(|t| now.duration_since(*t) >= LOGIN_WINDOW)
        {
            f.pop_front();
        }
        if f.len() < LOGIN_MAX_FAILURES {
            return None;
        }
        let oldest = *f.front()?;
        Some(LOGIN_WINDOW.saturating_sub(now.duration_since(oldest)))
    }

    pub fn login_failed(&self) {
        let mut f = self.login_failures.lock().unwrap();
        f.push_back(Instant::now());
        while f.len() > LOGIN_MAX_FAILURES {
            f.pop_front();
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

#[cfg(test)]
mod tests {
    use super::*;
    use rackscreen_app::logs::LogSink;

    fn state(pw: &str) -> AppState {
        let hub = Arc::new(Hub::new(
            "/nonexistent".into(),
            true,
            "0",
            LogSink::new(5),
            2,
        ));
        AppState::new(hub, pw.into(), 0)
    }

    #[test]
    fn five_failures_block_logins_for_a_minute() {
        let s = state("pw");
        for _ in 0..LOGIN_MAX_FAILURES - 1 {
            s.login_failed();
            assert!(s.login_blocked().is_none());
        }
        s.login_failed();
        let wait = s.login_blocked().unwrap();
        assert!(
            wait <= LOGIN_WINDOW && wait > Duration::from_secs(55),
            "{wait:?}"
        );
        let later = Instant::now() + LOGIN_WINDOW + Duration::from_secs(1);
        assert!(s.login_blocked_at(later).is_none(), "the window passed");
    }

    #[test]
    fn a_password_change_keeps_only_the_changing_session() {
        let s = state("old");
        let mine = s.new_session();
        let other = s.new_session();
        s.set_password("old", None);
        assert_eq!(s.session_count_for_test(), 2, "no change, no logout");
        s.set_password("new", Some(&mine));
        assert!(s.session_valid(&mine));
        assert!(!s.session_valid(&other));
        s.set_password("newer", None);
        assert_eq!(s.session_count_for_test(), 0);
    }
}
