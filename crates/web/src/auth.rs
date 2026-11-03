//! Optional password: a session cookie once logged in, and the `X-RackScreen`
//! header every non-GET request must carry (a cross-site form cannot set it).

use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::header::{COOKIE, SET_COOKIE};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use subtle::ConstantTimeEq;

use crate::{ApiError, ApiResult, AppState};

pub const COOKIE_NAME: &str = "rs_session";
pub const CSRF_HEADER: &str = "x-rackscreen";

/// The `rs_session` value from a `Cookie` header, if any.
pub fn session_from(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .map(str::trim)
        .find_map(|kv| {
            kv.strip_prefix(COOKIE_NAME)
                .and_then(|r| r.strip_prefix('='))
        })
        .map(str::to_string)
}

fn has_csrf_header(headers: &HeaderMap) -> bool {
    headers
        .get(CSRF_HEADER)
        .is_some_and(|v| v.as_bytes() == b"1")
}

fn constant_time_eq(a: &str, b: &str) -> bool {
    a.as_bytes().ct_eq(b.as_bytes()).into()
}

/// Both guards for the whole `/api` tree.
pub async fn guard(State(state): State<Arc<AppState>>, req: Request<Body>, next: Next) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let safe = matches!(method, Method::GET | Method::HEAD | Method::OPTIONS);
    if !safe && !has_csrf_header(req.headers()) {
        return ApiError(StatusCode::FORBIDDEN, "missing X-RackScreen header".into())
            .into_response();
    }
    // the router is nested under /api, so paths here have no prefix
    let open = (method == Method::GET && path == "/auth")
        || (method == Method::POST && path == "/auth/login");
    if state.login_required() && !open {
        let ok = session_from(req.headers()).is_some_and(|s| state.session_valid(&s));
        if !ok {
            return ApiError(StatusCode::UNAUTHORIZED, "login required".into()).into_response();
        }
    }
    next.run(req).await
}

pub async fn get_auth(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> ApiResult<serde_json::Value> {
    let required = state.login_required();
    let authenticated =
        !required || session_from(&headers).is_some_and(|s| state.session_valid(&s));
    Ok(Json(serde_json::json!({
        "required": required,
        "authenticated": authenticated,
    })))
}

#[derive(Deserialize)]
pub struct LoginBody {
    #[serde(default)]
    pub password: String,
}

fn cookie(id: &str, max_age: u64) -> HeaderValue {
    HeaderValue::from_str(&format!(
        "{COOKIE_NAME}={id}; Path=/; HttpOnly; SameSite=Strict; Max-Age={max_age}"
    ))
    .expect("ascii cookie")
}

pub async fn login(State(state): State<Arc<AppState>>, Json(body): Json<LoginBody>) -> Response {
    let ok = serde_json::json!({ "ok": true });
    let expected = state.password();
    if expected.is_empty() {
        return Json(ok).into_response();
    }
    if !constant_time_eq(&expected, &body.password) {
        return ApiError(StatusCode::UNAUTHORIZED, "wrong password".into()).into_response();
    }
    let id = state.new_session();
    let mut resp = Json(ok).into_response();
    resp.headers_mut()
        .insert(SET_COOKIE, cookie(&id, crate::state::SESSION_TTL.as_secs()));
    resp
}

pub async fn logout(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    if let Some(s) = session_from(&headers) {
        state.drop_session(&s);
    }
    let mut resp = Json(serde_json::json!({ "ok": true })).into_response();
    resp.headers_mut().insert(SET_COOKIE, cookie("", 0));
    resp
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_cookie_is_found_among_others() {
        let mut h = HeaderMap::new();
        h.insert(COOKIE, HeaderValue::from_static("a=1; rs_session=abc; b=2"));
        assert_eq!(session_from(&h).as_deref(), Some("abc"));
        let mut h = HeaderMap::new();
        h.insert(COOKIE, HeaderValue::from_static("rs_sessionx=abc"));
        assert_eq!(session_from(&h), None);
        assert_eq!(session_from(&HeaderMap::new()), None);
        assert!(constant_time_eq("hunter2", "hunter2"));
        assert!(!constant_time_eq("hunter2", "hunter3"));
        assert!(!constant_time_eq("hunter2", "hunter"));
    }
}
