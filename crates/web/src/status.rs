//! `/api/info` and `/api/status`.

use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use rackscreen_app::config::Config;
use serde_json::Value;

use crate::host;
use crate::{ApiResult, AppState};

pub async fn info(State(state): State<Arc<AppState>>) -> ApiResult<Value> {
    let hub = &state.hub;
    let writable = host::config_writable(&hub.config_path);
    let screens = Config::load_or_default(&hub.config_path)
        .map(|c| c.screens.len())
        .unwrap_or(0);
    Ok(Json(serde_json::json!({
        "version": hub.version,
        "hostname": host::hostname(),
        "sim": hub.sim,
        "started_unix": hub.started_unix,
        "config_path": hub.config_path.display().to_string(),
        "config_writable": writable,
        "fix_hint": if writable { Value::Null } else { Value::String(host::FIX_HINT.into()) },
        "addresses": host::addresses(state.port),
        "screens": screens,
    })))
}

pub async fn status(State(state): State<Arc<AppState>>) -> ApiResult<Value> {
    Ok(Json(status_json(&state)))
}

pub fn status_json(state: &AppState) -> Value {
    let snapshot = state.hub.status();
    let age_ms = state.hub.status_age_ms();
    let mut v = serde_json::to_value(&snapshot).expect("snapshot serialises");
    let obj = v.as_object_mut().expect("object");
    obj.insert(
        "monitor".into(),
        serde_json::to_value(state.hub.monitor()).expect("monitor serialises"),
    );
    obj.insert("age_ms".into(), age_ms.into());
    obj.insert(
        "host".into(),
        serde_json::to_value(host::host_info()).expect("host serialises"),
    );
    v
}
