//! `/api/config`: the config as JSON with secrets masked, saving with a hot
//! restart, the raw YAML, and validation without saving.

use std::sync::Arc;
use std::time::Duration;

use axum::extract::State;
use axum::Json;
use rackscreen_app::config::Config;
use rackscreen_app::hub::SupervisorCmd;
use serde::Deserialize;
use serde_json::{Map, Value};

use crate::{ApiError, ApiResult, AppState};

/// Dotted paths of the fields that never reach the browser.
pub const SECRET_PATHS: [&str; 8] = [
    "qbittorrent.pass",
    "electricity.token",
    "github.token",
    "media.radarr.api_key",
    "media.sonarr.api_key",
    "media.lidarr.api_key",
    "jellyfin.api_key",
    "web.password",
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
/// block changed), or nothing (no change).
pub fn restart_kind(old: &Config, new: &Config) -> &'static str {
    if old.web != new.web {
        "process"
    } else if old != new {
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

/// The incoming JSON merged with the stored secrets, normalised and validated.
fn parse_incoming(state: &AppState, mut incoming: Value) -> Result<(Config, Config), ApiError> {
    let stored = load_stored(state)?;
    let stored_json =
        serde_json::to_value(&stored).map_err(|e| ApiError::internal(e.to_string()))?;
    merge_secrets(&mut incoming, &stored_json);
    let mut cfg: Config = serde_json::from_value(incoming)
        .map_err(|e| ApiError::bad_request(format!("config: {e}")))?;
    cfg.normalize()?;
    cfg.validate()?;
    Ok((stored, cfg))
}

/// Save `cfg`, update the password, ask the supervisor, answer.
async fn save_and_restart(state: &Arc<AppState>, stored: Config, cfg: Config) -> ApiResult<Value> {
    let kind = restart_kind(&stored, &cfg);
    if kind != "none" {
        cfg.save(&state.hub.config_path)
            .map_err(|e| ApiError::internal(format!("{e:#}; {}", crate::host::FIX_HINT)))?;
        state.set_password(&cfg.web.password);
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
    Ok(Json(serde_json::json!({
        "config": config,
        "secrets": secrets,
        "restart": kind,
    })))
}

pub async fn put_config(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ConfigBody>,
) -> ApiResult<Value> {
    let _lock = state.config_lock.lock().await;
    let (stored, cfg) = parse_incoming(&state, body.config)?;
    save_and_restart(&state, stored, cfg).await
}

pub async fn validate(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ConfigBody>,
) -> ApiResult<Value> {
    Ok(Json(match parse_incoming(&state, body.config) {
        Ok(_) => serde_json::json!({ "ok": true }),
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
    Json(body): Json<YamlBody>,
) -> ApiResult<Value> {
    let _lock = state.config_lock.lock().await;
    let stored = load_stored(&state)?;
    let cfg = Config::from_yaml(&body.yaml)?;
    cfg.validate()?;
    save_and_restart(&state, stored, cfg).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_nulls_secrets_and_reports_which_are_set() {
        let mut cfg = Config::default();
        cfg.github.token = "ghp_x".into();
        cfg.web.password = "pw".into();
        let (v, secrets) = masked(&cfg).unwrap();
        assert_eq!(v["github"]["token"], Value::Null);
        assert_eq!(v["web"]["password"], Value::Null);
        assert_eq!(v["qbittorrent"]["pass"], Value::Null);
        assert_eq!(secrets["github.token"], true);
        assert_eq!(secrets["web.password"], true);
        assert_eq!(secrets["qbittorrent.pass"], false);
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
    }
}
