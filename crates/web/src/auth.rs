//! Optional password: a session cookie once logged in, the `X-RackScreen`
//! header every non-GET request must carry (a cross-site form cannot set it),
//! an `Origin` check against `Host`, and a global login rate limit.

use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::header::{COOKIE, HOST, ORIGIN, RETRY_AFTER, SET_COOKIE};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use sha2::{Digest, Sha256};
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

/// Compare the SHA-256 of both, so neither the length nor the content of the
/// password leaks through the timing.
fn constant_time_eq(a: &str, b: &str) -> bool {
    let ha = Sha256::digest(a.as_bytes());
    let hb = Sha256::digest(b.as_bytes());
    ha.as_slice().ct_eq(hb.as_slice()).into()
}

/// `host[:port]` with the default port of `scheme` filled in, lowercased.
fn authority_with_port(authority: &str, scheme: &str) -> Option<String> {
    let authority = authority.trim().to_ascii_lowercase();
    if authority.is_empty() || authority.contains(['/', '@', ' ']) {
        return None;
    }
    let digits = |p: &str| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit());
    // `[v6]:port`, `[v6]`, `host:port` or `host`
    let has_port = if let Some(rest) = authority.strip_prefix('[') {
        let (_, after) = rest.split_once(']')?;
        match after.strip_prefix(':') {
            None if after.is_empty() => false,
            Some(p) if digits(p) => true,
            _ => return None,
        }
    } else {
        match authority.rsplit_once(':') {
            Some((h, _)) if h.contains(':') => return None,
            Some((_, p)) if digits(p) => true,
            Some(_) => return None,
            None => false,
        }
    };
    if has_port {
        return Some(authority);
    }
    let port = match scheme {
        "http" | "ws" => 80,
        "https" | "wss" => 443,
        _ => return None,
    };
    Some(format!("{authority}:{port}"))
}

/// Whether an `Origin` header (if any) names the same host and port as
/// `Host`. No `Origin` passes (curl, same-origin GETs); `null` or anything
/// unparseable does not.
pub fn origin_matches(headers: &HeaderMap) -> bool {
    let Some(origin) = headers.get(ORIGIN) else {
        return true;
    };
    let (Ok(origin), Some(Ok(host))) = (origin.to_str(), headers.get(HOST).map(|h| h.to_str()))
    else {
        return false;
    };
    let Some((scheme, rest)) = origin.split_once("://") else {
        return false;
    };
    let scheme = scheme.to_ascii_lowercase();
    match (
        authority_with_port(rest.trim_end_matches('/'), &scheme),
        authority_with_port(host, &scheme),
    ) {
        (Some(o), Some(h)) => o == h,
        _ => false,
    }
}

/// Endpoints a cross-site page could watch with a plain GET: the frame
/// WebSocket and the log stream.
fn is_stream(path: &str) -> bool {
    path == "/live" || path == "/logs/stream"
}

/// Both guards for the whole `/api` tree.
pub async fn guard(State(state): State<Arc<AppState>>, req: Request<Body>, next: Next) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let safe = matches!(method, Method::GET | Method::HEAD | Method::OPTIONS);
    if (!safe || is_stream(&path)) && !origin_matches(req.headers()) {
        return ApiError(StatusCode::FORBIDDEN, "cross-origin request refused".into())
            .into_response();
    }
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

pub fn cookie(id: &str, max_age: u64) -> HeaderValue {
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
    if let Some(wait) = state.login_blocked() {
        let secs = wait.as_secs().max(1);
        let mut resp = ApiError(
            StatusCode::TOO_MANY_REQUESTS,
            format!("too many failed logins; try again in {secs} s"),
        )
        .into_response();
        resp.headers_mut()
            .insert(RETRY_AFTER, HeaderValue::from(secs));
        return resp;
    }
    if !constant_time_eq(&expected, &body.password) {
        state.login_failed();
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
        assert!(!constant_time_eq("", "x"));
    }

    fn hdrs(origin: Option<&'static str>, host: &'static str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(HOST, HeaderValue::from_static(host));
        if let Some(o) = origin {
            h.insert(ORIGIN, HeaderValue::from_static(o));
        }
        h
    }

    #[test]
    fn origin_must_name_the_host_and_port() {
        assert!(origin_matches(&hdrs(None, "pi:8740")));
        assert!(origin_matches(&hdrs(Some("http://pi:8740"), "pi:8740")));
        assert!(origin_matches(&hdrs(Some("http://PI:8740/"), "pi:8740")));
        assert!(origin_matches(&hdrs(Some("http://pi"), "pi")));
        assert!(origin_matches(&hdrs(Some("http://pi"), "pi:80")));
        assert!(origin_matches(&hdrs(Some("https://pi"), "pi:443")));
        assert!(origin_matches(&hdrs(
            Some("http://[fe80::1]:8740"),
            "[fe80::1]:8740"
        )));
        assert!(origin_matches(&hdrs(Some("http://[::1]"), "[::1]:80")));
        assert!(!origin_matches(&hdrs(Some("http://[::1]"), "[::1]:8740")));
        assert!(!origin_matches(&hdrs(Some("http://evil.com"), "pi:8740")));
        assert!(!origin_matches(&hdrs(Some("http://pi:9999"), "pi:8740")));
        assert!(!origin_matches(&hdrs(Some("http://pi"), "pi:8740")));
        assert!(!origin_matches(&hdrs(Some("null"), "pi:8740")));
        assert!(!origin_matches(&hdrs(
            Some("http://pi:8740@evil.com"),
            "pi:8740"
        )));
        let mut h = HeaderMap::new();
        h.insert(ORIGIN, HeaderValue::from_static("http://pi:8740"));
        assert!(!origin_matches(&h), "Origin without Host");
    }
}
