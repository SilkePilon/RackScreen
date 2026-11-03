//! `/api/config`: the config as JSON with secrets masked, saving with a hot
//! restart, the raw YAML, and validation without saving.

use std::sync::Arc;
use std::time::Duration;

use axum::extract::State;
use axum::http::header::SET_COOKIE;
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use axum::Json;
use rackscreen_app::config::Config;
use rackscreen_app::hub::SupervisorCmd;
use serde::Deserialize;
use serde_json::{Map, Value};

use crate::{ApiError, ApiResult, AppState};

/// Dotted paths of the fields that never reach the browser.
pub const SECRET_PATHS: [&str; 10] = [
    "qbittorrent.pass",
    "electricity.token",
    "github.token",
    "media.radarr.api_key",
    "media.sonarr.api_key",
    "media.lidarr.api_key",
    "jellyfin.api_key",
    "web.password",
    "gif_search.giphy_key",
    "gif_search.tenor_key",
];

fn get_path<'a>(v: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.').try_fold(v, |cur, k| cur.get(k))
}

fn get_path_mut<'a>(v: &'a mut Value, path: &str) -> Option<&'a mut Value> {
    path.split('.').try_fold(v, |cur, k| cur.get_mut(k))
}

/// Replace every secret with `null` and say which ones are set.
pub fn mask_secrets(config: &mut Value) -> Map<String, Value> {
    let mut secrets = Map::new();
    for path in SECRET_PATHS {
        let set = get_path(config, path)
            .and_then(Value::as_str)
            .is_some_and(|s| !s.is_empty());
        if let Some(slot) = get_path_mut(config, path) {
            *slot = Value::Null;
        }
        secrets.insert(path.to_string(), Value::Bool(set));
    }
    secrets
}

/// Fill the secrets of an incoming config from the stored one: `null` (or
/// a missing key) keeps the stored value, `""` clears it, any other string
/// sets it.
pub fn merge_secrets(incoming: &mut Value, stored: &Value) {
    for path in SECRET_PATHS {
        let keep = match get_path(incoming, path) {
            None | Some(Value::Null) => true,
            Some(Value::String(_)) => false,
            Some(_) => true,
        };
        if keep {
            let from = get_path(stored, path)
                .cloned()
                .unwrap_or(Value::String(String::new()));
            // the parent object exists once the config is the serde form;
            // create it when the client left a whole block out
            let (parent, key) = path.rsplit_once('.').expect("dotted path");
            let mut cur = &mut *incoming;
            for k in parent.split('.') {
                if !cur.is_object() {
                    *cur = Value::Object(Map::new());
                }
                cur = cur
                    .as_object_mut()
                    .unwrap()
                    .entry(k)
                    .or_insert_with(|| Value::Object(Map::new()));
            }
            if !cur.is_object() {
                *cur = Value::Object(Map::new());
            }
            cur.as_object_mut().unwrap().insert(key.to_string(), from);
        }
    }
}

/// `{ "config": ..., "secrets": ... }` for `cfg`.
pub fn masked(cfg: &Config) -> Result<(Value, Map<String, Value>), ApiError> {
    let mut v = serde_json::to_value(cfg).map_err(|e| ApiError::internal(e.to_string()))?;
    let secrets = mask_secrets(&mut v);
    Ok((v, secrets))
}

/// What a save needs: the monitor only, the whole process (the `web:`
/// block changed), or nothing (no change, or only `gif_search:`, which the
/// web server reads from the file on every search).
pub fn restart_kind(old: &Config, new: &Config) -> &'static str {
    let monitor_sees = Config {
        gif_search: new.gif_search.clone(),
        ..old.clone()
    };
    if old.web != new.web {
        "process"
    } else if monitor_sees != *new {
        "monitor"
    } else {
        "none"
    }
}

fn load_stored(state: &AppState) -> Result<Config, ApiError> {
    Config::load_or_default(&state.hub.config_path)
        .map_err(|e| ApiError::internal(format!("{e:#}")))
}

pub async fn get_config(State(state): State<Arc<AppState>>) -> ApiResult<Value> {
    let cfg = load_stored(&state)?;
    let (config, secrets) = masked(&cfg)?;
    Ok(Json(
        serde_json::json!({ "config": config, "secrets": secrets }),
    ))
}

#[derive(Deserialize)]
pub struct ConfigBody {
    pub config: Value,
}

/// Whether `bind:port` can be bound right now. Only asked when the address
/// changes; a port this server already listens on counts as free (only the
/// bind address moves, and the restart releases it first).
pub fn check_bind(old: &Config, new: &Config, listening_port: u16) -> Result<(), ApiError> {
    let (ob, nb) = (old.web.bind.trim(), new.web.bind.trim());
    if ob == nb && old.web.port == new.web.port {
        return Ok(());
    }
    if new.web.port == listening_port {
        return Ok(());
    }
    let ip: std::net::IpAddr = nb
        .parse()
        .map_err(|_| ApiError::bad_request(format!("web.bind must be an IP address (got {nb})")))?;
    let addr = std::net::SocketAddr::new(ip, new.web.port);
    match std::net::TcpListener::bind(addr) {
        Ok(l) => {
            drop(l);
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => Err(ApiError::bad_request(format!(
            "port {} is in use",
            new.web.port
        ))),
        Err(e) => Err(ApiError::bad_request(format!(
            "cannot listen on {addr}: {e}"
        ))),
    }
}

/// Every candidate config, from the form or the YAML editor, goes through
/// here before it is called valid or saved: validation plus the bind check.
/// Returns the restart a save would need.
fn check(state: &AppState, stored: &Config, cfg: &Config) -> Result<&'static str, ApiError> {
    cfg.validate()?;
    check_bind(stored, cfg, state.port)?;
    Ok(restart_kind(stored, cfg))
}

/// The incoming JSON merged with the stored secrets, normalised and checked.
fn parse_incoming(state: &AppState, mut incoming: Value) -> Result<(Config, Config), ApiError> {
    let stored = load_stored(state)?;
    let stored_json =
        serde_json::to_value(&stored).map_err(|e| ApiError::internal(e.to_string()))?;
    merge_secrets(&mut incoming, &stored_json);
    let mut cfg: Config = serde_json::from_value(incoming)
        .map_err(|e| ApiError::bad_request(format!("config: {e}")))?;
    cfg.normalize()?;
    check(state, &stored, &cfg)?;
    Ok((stored, cfg))
}

/// Raw YAML parsed (and normalised) the way the config file is, then checked.
fn parse_yaml(state: &AppState, yaml: &str) -> Result<(Config, Config), ApiError> {
    let stored = load_stored(state)?;
    let cfg = Config::from_yaml(yaml)?;
    check(state, &stored, &cfg)?;
    Ok((stored, cfg))
}

/// Save `cfg`, update the password, ask the supervisor, answer. A password
/// change keeps the session that made it (or starts one when the request
/// had none, e.g. the first password set) and ends every other.
async fn save_and_restart(
    state: &Arc<AppState>,
    headers: &HeaderMap,
    stored: Config,
    cfg: Config,
) -> Result<Response, ApiError> {
    let kind = restart_kind(&stored, &cfg);
    let mut new_cookie = None;
    if stored != cfg {
        cfg.save(&state.hub.config_path)
            .map_err(|e| ApiError::internal(format!("{e:#}; {}", crate::host::FIX_HINT)))?;
        let current = crate::auth::session_from(headers).filter(|s| state.session_valid(s));
        let changed = state.password() != cfg.web.password;
        state.set_password(&cfg.web.password, current.as_deref());
        if changed && current.is_none() && !cfg.web.password.is_empty() {
            new_cookie = Some(state.new_session());
        }
    }
    let (config, secrets) = masked(&cfg)?;
    match kind {
        "monitor" => {
            if !state.hub.request(SupervisorCmd::RestartMonitor) {
                tracing::warn!("config saved, but no supervisor to restart the monitor");
            }
        }
        "process" => {
            let hub = state.hub.clone();
            // let the answer leave first
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(300)).await;
                if !hub.request(SupervisorCmd::RestartProcess) {
                    tracing::warn!("config saved, but no supervisor to restart the process");
                }
            });
        }
        _ => {}
    }
    let mut resp = Json(serde_json::json!({
        "config": config,
        "secrets": secrets,
        "restart": kind,
    }))
    .into_response();
    if let Some(id) = new_cookie {
        resp.headers_mut().insert(
            SET_COOKIE,
            crate::auth::cookie(&id, crate::state::SESSION_TTL.as_secs()),
        );
    }
    Ok(resp)
}

pub async fn put_config(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<ConfigBody>,
) -> Result<Response, ApiError> {
    let _lock = state.config_lock.lock().await;
    let (stored, cfg) = parse_incoming(&state, body.config)?;
    save_and_restart(&state, &headers, stored, cfg).await
}

/// `{"config": ...}` or `{"yaml": "..."}`: the same checks as the save.
#[derive(Deserialize)]
pub struct ValidateBody {
    pub config: Option<Value>,
    pub yaml: Option<String>,
}

pub async fn validate(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ValidateBody>,
) -> ApiResult<Value> {
    let r = match (body.config, body.yaml) {
        (Some(c), _) => parse_incoming(&state, c),
        (None, Some(y)) => parse_yaml(&state, &y),
        (None, None) => Err(ApiError::bad_request("send config or yaml")),
    };
    Ok(Json(match r {
        Ok((stored, cfg)) => {
            serde_json::json!({ "ok": true, "restart": restart_kind(&stored, &cfg) })
        }
        Err(e) => serde_json::json!({ "ok": false, "error": e.1 }),
    }))
}

pub async fn get_yaml(State(state): State<Arc<AppState>>) -> ApiResult<Value> {
    let path = &state.hub.config_path;
    let yaml = if path.exists() {
        std::fs::read_to_string(path)
            .map_err(|e| ApiError::internal(format!("read config: {e}")))?
    } else {
        Config::default()
            .to_yaml()
            .map_err(|e| ApiError::internal(format!("{e:#}")))?
    };
    Ok(Json(serde_json::json!({ "yaml": yaml })))
}

#[derive(Deserialize)]
pub struct YamlBody {
    pub yaml: String,
}

pub async fn put_yaml(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<YamlBody>,
) -> Result<Response, ApiError> {
    let _lock = state.config_lock.lock().await;
    let (stored, cfg) = parse_yaml(&state, &body.yaml)?;
    save_and_restart(&state, &headers, stored, cfg).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_nulls_secrets_and_reports_which_are_set() {
        let mut cfg = Config::default();
        cfg.github.token = "ghp_x".into();
        cfg.web.password = "pw".into();
        cfg.gif_search.tenor_key = "AIza-tenor".into();
        let (v, secrets) = masked(&cfg).unwrap();
        assert_eq!(v["github"]["token"], Value::Null);
        assert_eq!(v["web"]["password"], Value::Null);
        assert_eq!(v["gif_search"]["tenor_key"], Value::Null);
        assert_eq!(v["gif_search"]["giphy_key"], Value::Null);
        assert_eq!(v["gif_search"]["rating"], "g");
        assert_eq!(secrets["gif_search.tenor_key"], true);
        assert!(!v.to_string().contains("AIza-tenor"));
        assert_eq!(v["qbittorrent"]["pass"], Value::Null);
        assert_eq!(secrets["github.token"], true);
        assert_eq!(secrets["web.password"], true);
        assert_eq!(secrets["qbittorrent.pass"], false);
        assert_eq!(secrets["gif_search.giphy_key"], false);
        assert_eq!(secrets.len(), SECRET_PATHS.len());
        assert_eq!(v["github"]["enabled"], false, "the rest is untouched");
        assert_eq!(v["screens"].as_array().unwrap().len(), 4);
    }

    #[test]
    fn merge_keeps_clears_and_sets_secrets() {
        let mut stored = Config::default();
        stored.github.token = "old".into();
        stored.jellyfin.api_key = "jf".into();
        stored.web.password = "pw".into();
        let stored_json = serde_json::to_value(&stored).unwrap();
        let (mut incoming, _) = masked(&stored).unwrap();
        // null keeps, "" clears, a string sets
        incoming["jellyfin"]["api_key"] = Value::String(String::new());
        incoming["electricity"]["token"] = Value::String("new".into());
        // a block the client left out entirely still gets its stored secret
        incoming.as_object_mut().unwrap().remove("web");
        merge_secrets(&mut incoming, &stored_json);
        assert_eq!(incoming["github"]["token"], "old");
        assert_eq!(incoming["jellyfin"]["api_key"], "");
        assert_eq!(incoming["electricity"]["token"], "new");
        assert_eq!(incoming["web"]["password"], "pw");
        let cfg: Config = serde_json::from_value(incoming).unwrap();
        assert_eq!(cfg.github.token, "old");
        assert_eq!(cfg.web.password, "pw");
        assert_eq!(cfg.web.port, 8740, "the rest of the block is the default");
    }

    #[test]
    fn bind_check_only_probes_a_changed_address() {
        let held = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let busy = held.local_addr().unwrap().port();
        let mut old = Config::default();
        old.web.bind = "127.0.0.1".into();
        old.web.port = 1;
        assert!(check_bind(&old, &old, 0).is_ok(), "unchanged: no probe");
        let mut new = old.clone();
        new.web.port = busy;
        let e = check_bind(&old, &new, 0).unwrap_err();
        assert_eq!(e.0, axum::http::StatusCode::BAD_REQUEST);
        assert_eq!(e.1, format!("port {busy} is in use"));
        assert!(
            check_bind(&old, &new, busy).is_ok(),
            "the port this server listens on is ours"
        );
        drop(held);
        let free = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        new.web.port = free.local_addr().unwrap().port();
        drop(free);
        assert!(check_bind(&old, &new, 0).is_ok());
    }

    #[test]
    fn restart_kind_follows_what_changed() {
        let a = Config::default();
        assert_eq!(restart_kind(&a, &a), "none");
        let mut b = a.clone();
        b.display.fps = 30;
        assert_eq!(restart_kind(&a, &b), "monitor");
        let mut c = a.clone();
        c.web.port = 1;
        assert_eq!(restart_kind(&a, &c), "process");
        let mut d = b.clone();
        d.web.password = "x".into();
        assert_eq!(restart_kind(&a, &d), "process");
        let mut e = a.clone();
        e.gif_search.giphy_key = "k".into();
        assert_eq!(
            restart_kind(&a, &e),
            "none",
            "the search keys need no restart"
        );
    }
}
