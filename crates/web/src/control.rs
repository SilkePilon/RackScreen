//! `/api/control/*`: identify, test pattern, jump to a role, play a show,
//! night override and a hot restart.

use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use rackscreen_app::config::Config;
use rackscreen_app::hub::{Control, ControlHandle, SupervisorCmd};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{ApiError, ApiResult, AppState};

fn handle(state: &AppState) -> Result<ControlHandle, ApiError> {
    state
        .hub
        .control()
        .ok_or_else(|| ApiError::unavailable("the monitor is not running"))
}

fn send(state: &AppState, c: Control) -> ApiResult<Value> {
    handle(state)?
        .control
        .send(c)
        .map_err(|_| ApiError::unavailable("the render loop is gone"))?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct IdentifyBody {
    #[serde(default = "default_secs")]
    pub secs: f64,
}

fn default_secs() -> f64 {
    5.0
}

pub async fn identify(
    State(state): State<Arc<AppState>>,
    Json(body): Json<IdentifyBody>,
) -> ApiResult<Value> {
    if !body.secs.is_finite() || body.secs <= 0.0 {
        return Err(ApiError::bad_request("secs must be positive"));
    }
    send(&state, Control::Identify { secs: body.secs })
}

#[derive(Deserialize)]
pub struct PatternBody {
    pub on: bool,
}

pub async fn pattern(
    State(state): State<Arc<AppState>>,
    Json(body): Json<PatternBody>,
) -> ApiResult<Value> {
    send(&state, Control::Pattern { on: body.on })
}

#[derive(Deserialize)]
pub struct RoleBody {
    pub screen: usize,
    pub role: String,
}

pub async fn role(
    State(state): State<Arc<AppState>>,
    Json(body): Json<RoleBody>,
) -> ApiResult<Value> {
    let cfg = Config::load_or_default(&state.hub.config_path)
        .map_err(|e| ApiError::internal(format!("{e:#}")))?;
    let screen = cfg
        .screens
        .get(body.screen)
        .ok_or_else(|| ApiError::bad_request(format!("no screen {}", body.screen)))?;
    let roles = cfg.screen_roles(screen)?;
    let role = roles
        .iter()
        .copied()
        .find(|r| cfg.role_name(*r) == body.role)
        .ok_or_else(|| {
            ApiError::bad_request(format!(
                "screen {} does not list '{}'",
                body.screen + 1,
                body.role
            ))
        })?;
    send(
        &state,
        Control::Jump {
            screen: body.screen,
            role,
        },
    )
}

#[derive(Deserialize)]
pub struct PlayBody {
    pub show: String,
}

pub async fn play(
    State(state): State<Arc<AppState>>,
    Json(body): Json<PlayBody>,
) -> ApiResult<Value> {
    let req = rackscreen_app::play::parse(&body.show)?;
    send(&state, Control::Play(req))
}

#[derive(Deserialize)]
pub struct NightBody {
    pub mode: String,
}

pub async fn night(
    State(state): State<Arc<AppState>>,
    Json(body): Json<NightBody>,
) -> ApiResult<Value> {
    let v = match body.mode.as_str() {
        "auto" => None,
        "on" => Some(true),
        "off" => Some(false),
        other => {
            return Err(ApiError::bad_request(format!(
                "mode must be auto, on or off (got {other})"
            )))
        }
    };
    // kept in the hub, so a monitor restart (every config save) keeps it;
    // without a running loop it applies once the monitor is back
    let applied = state.hub.set_night_override(v);
    Ok(Json(json!({ "ok": true, "applied": applied })))
}

pub async fn restart(State(state): State<Arc<AppState>>) -> ApiResult<Value> {
    if !state.hub.request(SupervisorCmd::RestartMonitor) {
        return Err(ApiError::unavailable("no supervisor is listening"));
    }
    Ok(Json(json!({ "ok": true })))
}
