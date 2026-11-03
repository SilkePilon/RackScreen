//! `/api/spotify`: the PKCE login done from the browser. The pasted address
//! is parsed, never fetched; the only requests go to Spotify's token and
//! profile endpoints.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use axum::extract::State;
use axum::Json;
use rackscreen_app::config::Config;
use rackscreen_sources::spotify::{
    apply_token_response, code_form, login_status, new_login, parse_display_name, parse_redirect,
    read_token, write_token, TokenFile, ME_URL, TOKEN_URL,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::state::{PendingLogin, SPOTIFY_LOGIN_TTL};
use crate::{ApiError, ApiResult, AppState};

fn token_path() -> Result<PathBuf, ApiError> {
    rackscreen_app::spotify::token_path()
        .ok_or_else(|| ApiError::internal("no home directory for the token file"))
}

/// `logged in as X` with a capital, for the UI.
pub fn status_text(tok: Option<&TokenFile>) -> String {
    let s = login_status(tok);
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => s,
    }
}

fn status_json(cfg: &Config) -> Result<Value, ApiError> {
    let tok = read_token(&token_path()?);
    Ok(json!({
        "enabled": cfg.spotify.enabled,
        "client_id_set": !cfg.spotify.client_id.trim().is_empty(),
        "logged_in": tok.as_ref().is_some_and(|t| t.error.is_none()),
        "status": status_text(tok.as_ref()),
    }))
}

fn load(state: &AppState) -> Result<Config, ApiError> {
    Config::load_or_default(&state.hub.config_path)
        .map_err(|e| ApiError::internal(format!("{e:#}")))
}

pub async fn get_spotify(State(state): State<Arc<AppState>>) -> ApiResult<Value> {
    Ok(Json(status_json(&load(&state)?)?))
}

pub async fn login(State(state): State<Arc<AppState>>) -> ApiResult<Value> {
    let cfg = load(&state)?;
    let client_id = cfg.spotify.client_id.trim().to_string();
    if client_id.is_empty() {
        return Err(ApiError::bad_request("set spotify.client_id first"));
    }
    let mut pending = state.spotify.lock().unwrap();
    let fresh = pending
        .as_ref()
        .is_some_and(|p| p.client_id == client_id && p.created.elapsed() < SPOTIFY_LOGIN_TTL);
    if !fresh {
        *pending = Some(PendingLogin {
            login: new_login(&client_id)?,
            client_id,
            created: Instant::now(),
        });
    }
    let url = pending.as_ref().expect("just set").login.url.clone();
    Ok(Json(json!({ "url": url })))
}

#[derive(Deserialize)]
pub struct CallbackBody {
    pub pasted: String,
}

fn unix_now() -> i64 {
    rackscreen_app::hub::unix_now() as i64
}

/// The token endpoint, then `/v1/me` for the display name.
async fn exchange(client_id: &str, code: &str, verifier: &str) -> anyhow::Result<TokenFile> {
    let http = rackscreen_sources::http::client();
    let resp = http
        .post(TOKEN_URL)
        .form(&code_form(client_id, code, verifier))
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("reach accounts.spotify.com: {e}"))?;
    let status = resp.status().as_u16();
    let body = resp.text().await.unwrap_or_default();
    let mut tok = apply_token_response(status, &body, None, unix_now())
        .map_err(|e| anyhow::anyhow!("Spotify refused the code: {e}"))?;
    if let Ok(me) = http.get(ME_URL).bearer_auth(&tok.access_token).send().await {
        tok.display_name = parse_display_name(&me.text().await.unwrap_or_default());
    }
    Ok(tok)
}

pub async fn callback(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CallbackBody>,
) -> ApiResult<Value> {
    let (client_id, code, verifier) = {
        let pending = state.spotify.lock().unwrap();
        let p = pending
            .as_ref()
            .filter(|p| p.created.elapsed() < SPOTIFY_LOGIN_TTL)
            .ok_or_else(|| {
                ApiError::bad_request("start the login first (it expires after 10 min)")
            })?;
        let code = parse_redirect(&body.pasted, &p.login.state)
            .map_err(|e| ApiError::bad_request(e.to_string()))?;
        (p.client_id.clone(), code, p.login.verifier.clone())
    };
    let tok = exchange(&client_id, &code, &verifier)
        .await
        .map_err(|e| ApiError::bad_request(format!("{e:#}")))?;
    let path = token_path()?;
    write_token(&path, &tok, None).map_err(|e| ApiError::internal(format!("{e:#}")))?;
    *state.spotify.lock().unwrap() = None;
    tracing::info!("spotify: {}", login_status(Some(&tok)));
    Ok(Json(
        json!({ "ok": true, "status": status_text(Some(&tok)) }),
    ))
}

pub async fn logout(State(state): State<Arc<AppState>>) -> ApiResult<Value> {
    let path = token_path()?;
    match std::fs::remove_file(&path) {
        Ok(()) => tracing::info!("spotify: logged out"),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => {
            return Err(ApiError::internal(format!(
                "remove {}: {e}",
                path.display()
            )))
        }
    }
    *state.spotify.lock().unwrap() = None;
    Ok(Json(json!({ "ok": true })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_text_is_capitalised() {
        assert_eq!(status_text(None), "Not logged in");
        let tok = TokenFile {
            access_token: "a".into(),
            refresh_token: "r".into(),
            expires_at: 1,
            display_name: Some("Silke".into()),
            error: None,
        };
        assert_eq!(status_text(Some(&tok)), "Logged in as Silke");
    }
}
