//! `/api/control/*`: identify, test pattern, jump to a role, play a show,
//! a transition test, night override and a hot restart.

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

#[derive(Deserialize, Default)]
pub struct TransitionTestBody {
    /// One screen (0-based), or every cycling screen when left out.
    pub screen: Option<usize>,
}

/// Why a transition test cannot run right now, from the latest status.
pub fn transition_test_refusal(
    showing: Option<&str>,
    screen: Option<usize>,
    roles: impl Fn(usize) -> Option<usize>,
    screens: usize,
) -> Option<String> {
    match showing {
        Some("show") => return Some("Wait for the show to finish first.".into()),
        Some("night") => return Some("The panels are asleep: switch night mode off first.".into()),
        Some("pattern" | "identify") => {
            return Some("The test pattern is on: turn it off first.".into())
        }
        _ => {}
    }
    match screen {
        Some(i) if i >= screens => Some(format!("There is no screen {}.", i + 1)),
        Some(i) if roles(i).unwrap_or(0) < 2 => Some(format!(
            "Screen {} shows one role, so it has nothing to change to.",
            i + 1
        )),
        None if (0..screens).all(|i| roles(i).unwrap_or(0) < 2) => {
            Some("No screen cycles roles: give a screen two or more roles first.".into())
        }
        _ => None,
    }
}

/// Make a screen (or every cycling screen, left to right) change to its
/// next role now, in the saved transition style.
pub async fn transition_test(
    State(state): State<Arc<AppState>>,
    body: Option<Json<TransitionTestBody>>,
) -> ApiResult<Value> {
    let body = body.map(|Json(b)| b).unwrap_or_default();
    let status = state.hub.status();
    let showing = status.screens.first().map(|s| s.showing);
    let roles = |i: usize| status.screens.get(i).map(|s| s.roles.len());
    if let Some(why) = transition_test_refusal(showing, body.screen, roles, status.screens.len()) {
        return Err(ApiError::conflict(why));
    }
    send(
        &state,
        Control::TransitionTest {
            screen: body.screen,
        },
    )
}

pub async fn restart(State(state): State<Arc<AppState>>) -> ApiResult<Value> {
    if !state.hub.request(SupervisorCmd::RestartMonitor) {
        return Err(ApiError::unavailable("no supervisor is listening"));
    }
    Ok(Json(json!({ "ok": true })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transition_test_refusals() {
        let roles = |i: usize| [1usize, 3, 2].get(i).copied();
        let r = |showing, screen| transition_test_refusal(showing, screen, roles, 3);
        assert!(r(Some("show"), None).unwrap().contains("show"));
        assert!(r(Some("night"), Some(1)).unwrap().contains("night"));
        assert!(r(Some("pattern"), None).unwrap().contains("pattern"));
        assert!(r(Some("identify"), None).unwrap().contains("pattern"));
        assert_eq!(r(Some("role"), None), None);
        assert_eq!(r(Some("role"), Some(1)), None);
        assert!(r(Some("role"), Some(0)).unwrap().contains("one role"));
        assert!(r(Some("role"), Some(7)).unwrap().contains("no screen 8"));
        // no status yet (the monitor just started): let the loop decide
        assert_eq!(r(None, None), None);
        let none = |_: usize| Some(1usize);
        assert!(transition_test_refusal(Some("role"), None, none, 3)
            .unwrap()
            .contains("No screen cycles"));
    }
}
