//! The HTTP API through the router (no socket): guards, config round trip,
//! status shape, logs, static fallback.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use rackscreen_app::config::Config;
use rackscreen_app::hub::{Hub, SupervisorCmd};
use rackscreen_app::logs::LogSink;
use rackscreen_web::{router, AppState};
use serde_json::{json, Value};
use tower::ServiceExt;

struct Fixture {
    _dir: tempfile::TempDir,
    hub: Arc<Hub>,
    state: Arc<AppState>,
    app: Router,
}

fn fixture(password: &str) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.yaml");
    let mut cfg = Config::default();
    cfg.web.password = password.into();
    cfg.github.token = "ghp_stored".into();
    cfg.save(&path).unwrap();
    let hub = Arc::new(Hub::new(path, true, "9.9.9", LogSink::new(50), 8));
    let state = Arc::new(AppState::new(hub.clone(), password.into(), 8740));
    let app = router(state.clone());
    Fixture {
        _dir: dir,
        hub,
        state,
        app,
    }
}

async fn send(app: &Router, req: Request<Body>) -> (StatusCode, header::HeaderMap, Value) {
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let headers = resp.headers().clone();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, headers, json)
}

fn get(path: &str) -> Request<Body> {
    Request::get(path).body(Body::empty()).unwrap()
}

fn post_json(
    method: &str,
    path: &str,
    body: Value,
    csrf: bool,
    cookie: Option<&str>,
) -> Request<Body> {
    let mut b = Request::builder()
        .method(method)
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json");
    if csrf {
        b = b.header("X-RackScreen", "1");
    }
    if let Some(c) = cookie {
        b = b.header(header::COOKIE, format!("rs_session={c}"));
    }
    b.body(Body::from(body.to_string())).unwrap()
}

#[tokio::test]
async fn non_get_without_the_header_is_forbidden() {
    let f = fixture("");
    let (st, _, body) = send(
        &f.app,
        post_json(
            "POST",
            "/api/control/night",
            json!({"mode": "on"}),
            false,
            None,
        ),
    )
    .await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    assert!(body["error"].as_str().unwrap().contains("X-RackScreen"));
    // with the header and no monitor running the control is unavailable, not forbidden
    let (st, _, _) = send(
        &f.app,
        post_json(
            "POST",
            "/api/control/night",
            json!({"mode": "on"}),
            true,
            None,
        ),
    )
    .await;
    assert_eq!(st, StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn password_gates_the_api_and_login_sets_a_session_cookie() {
    let f = fixture("hunter2");
    let (st, _, body) = send(&f.app, get("/api/auth")).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(body, json!({"required": true, "authenticated": false}));
    let (st, _, _) = send(&f.app, get("/api/status")).await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);
    let (st, _, _) = send(
        &f.app,
        post_json(
            "POST",
            "/api/auth/login",
            json!({"password": "nope"}),
            true,
            None,
        ),
    )
    .await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);
    let (st, headers, body) = send(
        &f.app,
        post_json(
            "POST",
            "/api/auth/login",
            json!({"password": "hunter2"}),
            true,
            None,
        ),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(body, json!({"ok": true}));
    let cookie = headers[header::SET_COOKIE].to_str().unwrap().to_string();
    assert!(
        cookie.contains("HttpOnly") && cookie.contains("SameSite=Strict"),
        "{cookie}"
    );
    let id = cookie
        .split(';')
        .next()
        .unwrap()
        .strip_prefix("rs_session=")
        .unwrap()
        .to_string();
    assert_eq!(id.len(), 64);
    let req = Request::get("/api/status")
        .header(header::COOKIE, format!("rs_session={id}"))
        .body(Body::empty())
        .unwrap();
    let (st, _, body) = send(&f.app, req).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(body["monitor"]["state"], "starting");
    let (st, _, _) = send(
        &f.app,
        post_json("POST", "/api/auth/logout", json!({}), true, Some(&id)),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let req = Request::get("/api/status")
        .header(header::COOKIE, format!("rs_session={id}"))
        .body(Body::empty())
        .unwrap();
    let (st, _, _) = send(&f.app, req).await;
    assert_eq!(st, StatusCode::UNAUTHORIZED, "logged out");
    assert_eq!(f.state.session_count_for_test(), 0);
}

#[tokio::test]
async fn status_has_the_documented_shape() {
    let f = fixture("");
    f.hub.set_monitor("running", None);
    let (st, _, body) = send(&f.app, get("/api/status")).await;
    assert_eq!(st, StatusCode::OK);
    for key in [
        "monitor", "night", "screens", "links", "stats", "tiles", "host",
    ] {
        assert!(body.get(key).is_some(), "{key} missing: {body}");
    }
    assert_eq!(body["monitor"]["state"], "running");
    assert_eq!(body["night"]["mode"], "auto");
    assert!(body["screens"].is_array() && body["tiles"].is_array());
    assert!(body["host"].get("cpu_temp_c").is_some());
    let (st, _, body) = send(&f.app, get("/api/info")).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(body["version"], "9.9.9");
    assert_eq!(body["sim"], true);
    assert_eq!(body["screens"], 4);
    assert_eq!(body["config_writable"], true);
    assert_eq!(body["fix_hint"], Value::Null);
    assert!(body["addresses"].is_array());
    let (st, _, body) = send(&f.app, get("/api/roles")).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(body.as_array().unwrap().len(), 29);
    let (_, _, body) = send(&f.app, get("/api/shows")).await;
    assert_eq!(body[0]["id"], "freight");
}

#[tokio::test]
async fn config_round_trip_masks_merges_and_asks_for_a_restart() {
    let f = fixture("");
    let rx = f.hub.supervisor_mailbox();
    let (st, _, body) = send(&f.app, get("/api/config")).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(body["config"]["github"]["token"], Value::Null);
    assert_eq!(body["secrets"]["github.token"], true);
    assert_eq!(body["secrets"]["web.password"], false);
    assert_eq!(body["config"]["web"]["port"], 8740);
    let mut cfg = body["config"].clone();
    cfg["display"]["fps"] = json!(30);
    cfg["jellyfin"]["api_key"] = json!("jf-new");
    let (st, _, body) = send(
        &f.app,
        post_json("PUT", "/api/config", json!({"config": cfg}), true, None),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{body}");
    assert_eq!(body["restart"], "monitor");
    assert_eq!(body["config"]["display"]["fps"], 30);
    assert_eq!(body["config"]["jellyfin"]["api_key"], Value::Null);
    assert_eq!(body["secrets"]["jellyfin.api_key"], true);
    assert_eq!(rx.try_recv().unwrap(), SupervisorCmd::RestartMonitor);
    let stored = Config::load_or_default(&f.hub.config_path).unwrap();
    assert_eq!(stored.display.fps, 30);
    assert_eq!(
        stored.github.token, "ghp_stored",
        "null kept the stored secret"
    );
    assert_eq!(stored.jellyfin.api_key, "jf-new");
    // nothing changed: no restart
    let (_, _, body) = send(
        &f.app,
        post_json("PUT", "/api/config", json!({"config": cfg}), true, None),
    )
    .await;
    assert_eq!(body["restart"], "none");
    assert!(rx.try_recv().is_err());
    // an invalid config is refused and not saved
    let mut bad = cfg.clone();
    bad["screens"][0]["roles"] = json!(["foo"]);
    let (st, _, body) = send(
        &f.app,
        post_json("PUT", "/api/config", json!({"config": bad}), true, None),
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().unwrap().contains("foo"), "{body}");
    assert_eq!(
        Config::load_or_default(&f.hub.config_path).unwrap().screens[0].roles,
        vec!["cpu"]
    );
    let (_, _, body) = send(
        &f.app,
        post_json(
            "POST",
            "/api/config/validate",
            json!({"config": bad}),
            true,
            None,
        ),
    )
    .await;
    assert_eq!(body["ok"], false);
    let (_, _, body) = send(
        &f.app,
        post_json(
            "POST",
            "/api/config/validate",
            json!({"config": cfg}),
            true,
            None,
        ),
    )
    .await;
    assert_eq!(body["ok"], true, "{body}");
    // the web block: a process restart, and the password takes effect
    let mut web = cfg.clone();
    web["web"]["password"] = json!("newpw");
    let (st, _, body) = send(
        &f.app,
        post_json("PUT", "/api/config", json!({"config": web}), true, None),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(body["restart"], "process");
    assert_eq!(body["secrets"]["web.password"], true);
    assert_eq!(body["config"]["web"]["password"], Value::Null);
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    assert_eq!(rx.try_recv().unwrap(), SupervisorCmd::RestartProcess);
    let (st, _, _) = send(&f.app, get("/api/status")).await;
    assert_eq!(
        st,
        StatusCode::UNAUTHORIZED,
        "the new password applies at once"
    );
}

#[tokio::test]
async fn yaml_round_trip() {
    let f = fixture("");
    let _rx = f.hub.supervisor_mailbox();
    let (st, _, body) = send(&f.app, get("/api/config/yaml")).await;
    assert_eq!(st, StatusCode::OK);
    let yaml = body["yaml"].as_str().unwrap();
    assert!(
        yaml.contains("ghp_stored"),
        "secrets included in the raw yaml"
    );
    let yaml = yaml.replace("fps: 60", "fps: 25");
    let (st, _, body) = send(
        &f.app,
        post_json("PUT", "/api/config/yaml", json!({"yaml": yaml}), true, None),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{body}");
    assert_eq!(body["restart"], "monitor");
    assert_eq!(body["config"]["display"]["fps"], 25);
    let (st, _, body) = send(
        &f.app,
        post_json(
            "PUT",
            "/api/config/yaml",
            json!({"yaml": "screens: [}"}),
            true,
            None,
        ),
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().unwrap().contains("parse"), "{body}");
}

#[tokio::test]
async fn logs_page_after_a_sequence_number() {
    let f = fixture("");
    for i in 0..5 {
        f.hub.logs.push("INFO", "t", &format!("line {i}"));
    }
    let (st, _, body) = send(&f.app, get("/api/logs?after=2&limit=2")).await;
    assert_eq!(st, StatusCode::OK);
    let lines = body["lines"].as_array().unwrap();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0]["seq"], 3);
    assert_eq!(lines[0]["msg"], "line 2");
    assert_eq!(lines[0]["level"], "INFO");
    assert_eq!(lines[1]["seq"], 4);
    assert_eq!(body["next"], 4);
    let (_, _, body) = send(&f.app, get("/api/logs")).await;
    assert_eq!(body["lines"].as_array().unwrap().len(), 5);
    assert_eq!(body["next"], 5);
}

#[tokio::test]
async fn static_fallback_serves_the_shell_and_api_404s_are_json() {
    let f = fixture("");
    let resp = f.app.clone().oneshot(get("/")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(resp.headers()[header::CONTENT_TYPE]
        .to_str()
        .unwrap()
        .starts_with("text/html"));
    assert_eq!(resp.headers()[header::CACHE_CONTROL], "no-cache");
    let resp = f.app.clone().oneshot(get("/screens/2")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "SPA route");
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    assert!(
        std::str::from_utf8(&body)
            .unwrap()
            .contains("<!doctype html>")
            || std::str::from_utf8(&body)
                .unwrap()
                .contains("<!DOCTYPE html>")
    );
    let (st, _, body) = send(&f.app, get("/api/nope")).await;
    assert_eq!(st, StatusCode::NOT_FOUND);
    assert_eq!(body, json!({"error": "no such endpoint"}));
    let resp = f
        .app
        .clone()
        .oneshot(get("/assets/missing-abc.js"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn control_endpoints_validate_before_reaching_the_loop() {
    let f = fixture("");
    let (st, _, body) = send(
        &f.app,
        post_json(
            "POST",
            "/api/control/role",
            json!({"screen": 0, "role": "weather"}),
            true,
            None,
        ),
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().unwrap().contains("weather"));
    let (st, _, _) = send(
        &f.app,
        post_json(
            "POST",
            "/api/control/role",
            json!({"screen": 9, "role": "cpu"}),
            true,
            None,
        ),
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    let (st, _, _) = send(
        &f.app,
        post_json(
            "POST",
            "/api/control/play",
            json!({"show": "nope"}),
            true,
            None,
        ),
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    let (st, _, _) = send(
        &f.app,
        post_json(
            "POST",
            "/api/control/night",
            json!({"mode": "dusk"}),
            true,
            None,
        ),
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    let (st, _, _) = send(
        &f.app,
        post_json(
            "POST",
            "/api/control/identify",
            json!({"secs": -1}),
            true,
            None,
        ),
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    let (st, _, _) = send(
        &f.app,
        post_json("POST", "/api/control/restart", json!({}), true, None),
    )
    .await;
    assert_eq!(st, StatusCode::SERVICE_UNAVAILABLE, "no supervisor");
    let _rx = f.hub.supervisor_mailbox();
    let (st, _, _) = send(
        &f.app,
        post_json("POST", "/api/control/restart", json!({}), true, None),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let (st, _, body) = send(
        &f.app,
        post_json("POST", "/api/spotify/login", json!({}), true, None),
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().unwrap().contains("client_id"));
    let (st, _, _) = send(
        &f.app,
        post_json(
            "POST",
            "/api/spotify/callback",
            json!({"pasted": "http://127.0.0.1:8888/callback?code=x"}),
            true,
            None,
        ),
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST, "no login started");
}

#[tokio::test]
async fn a_foreign_origin_cannot_watch_or_write() {
    let f = fixture("");
    let with = |method: &str, path: &str, origin: &str| {
        Request::builder()
            .method(method)
            .uri(path)
            .header(header::HOST, "pi:8740")
            .header(header::ORIGIN, origin)
            .header(header::CONTENT_TYPE, "application/json")
            .header("X-RackScreen", "1")
            .body(Body::from("{}"))
            .unwrap()
    };
    // the stream endpoints refuse a foreign Origin before anything else
    for path in ["/api/live", "/api/logs/stream"] {
        let (st, _, body) = send(&f.app, with("GET", path, "http://evil.example")).await;
        assert_eq!(st, StatusCode::FORBIDDEN, "{path}");
        assert!(body["error"].as_str().unwrap().contains("cross-origin"));
    }
    // the stream never ends: look at the status only
    let resp = f
        .app
        .clone()
        .oneshot(with("GET", "/api/logs/stream", "http://pi:8740"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "same origin streams");
    // any write with a foreign Origin, even with the X-RackScreen header
    let (st, _, _) = send(
        &f.app,
        with("POST", "/api/control/restart", "http://pi:9999"),
    )
    .await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    let (st, _, _) = send(&f.app, with("POST", "/api/auth/logout", "http://pi:8740")).await;
    assert_eq!(st, StatusCode::OK);
    // plain GETs from elsewhere are left to CORS (no Access-Control headers)
    let (st, _, _) = send(&f.app, with("GET", "/api/info", "http://evil.example")).await;
    assert_eq!(st, StatusCode::OK);
}

#[tokio::test]
async fn repeated_wrong_passwords_get_429_with_retry_after() {
    let f = fixture("hunter2");
    let login = |pw: &str| {
        post_json(
            "POST",
            "/api/auth/login",
            json!({"password": pw}),
            true,
            None,
        )
    };
    for _ in 0..5 {
        let (st, _, _) = send(&f.app, login("nope")).await;
        assert_eq!(st, StatusCode::UNAUTHORIZED);
    }
    let (st, headers, _) = send(&f.app, login("hunter2")).await;
    assert_eq!(
        st,
        StatusCode::TOO_MANY_REQUESTS,
        "even the right one waits"
    );
    let secs: u64 = headers[header::RETRY_AFTER]
        .to_str()
        .unwrap()
        .parse()
        .unwrap();
    assert!((1..=60).contains(&secs), "{secs}");
}

#[tokio::test]
async fn changing_the_password_keeps_the_session_that_did_it() {
    let f = fixture("old");
    let _rx = f.hub.supervisor_mailbox();
    let mine = f.state.new_session();
    let other = f.state.new_session();
    let (_, _, body) = send(
        &f.app,
        Request::get("/api/config")
            .header(header::COOKIE, format!("rs_session={mine}"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    let mut config = body["config"].clone();
    config["web"]["password"] = json!("new");
    let (st, _, body) = send(
        &f.app,
        post_json(
            "PUT",
            "/api/config",
            json!({"config": config}),
            true,
            Some(&mine),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{body}");
    assert!(f.state.session_valid(&mine));
    assert!(!f.state.session_valid(&other));
}

#[tokio::test]
async fn setting_the_first_password_logs_the_setter_in() {
    let f = fixture("");
    let _rx = f.hub.supervisor_mailbox();
    let (_, _, body) = send(&f.app, get("/api/config")).await;
    let mut config = body["config"].clone();
    config["web"]["password"] = json!("pw");
    let (st, headers, _) = send(
        &f.app,
        post_json("PUT", "/api/config", json!({"config": config}), true, None),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let cookie = headers[header::SET_COOKIE].to_str().unwrap();
    let id = cookie
        .split(';')
        .next()
        .unwrap()
        .strip_prefix("rs_session=")
        .unwrap();
    assert!(f.state.session_valid(id));
}

#[tokio::test]
async fn yaml_goes_through_the_same_checks_as_the_form() {
    let f = fixture("");
    let held = std::net::TcpListener::bind("0.0.0.0:0").unwrap();
    let busy = held.local_addr().unwrap().port();
    let (_, _, body) = send(&f.app, get("/api/config/yaml")).await;
    let yaml = body["yaml"]
        .as_str()
        .unwrap()
        .replace("port: 8740", &format!("port: {busy}"));
    assert!(yaml.contains(&format!("port: {busy}")));
    let (st, _, body) = send(
        &f.app,
        post_json(
            "POST",
            "/api/config/validate",
            json!({"yaml": yaml}),
            true,
            None,
        ),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(body["ok"], false);
    assert_eq!(body["error"], format!("port {busy} is in use"));
    let (st, _, body) = send(
        &f.app,
        post_json("PUT", "/api/config/yaml", json!({"yaml": yaml}), true, None),
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], format!("port {busy} is in use"));
    let (_, _, body) = send(
        &f.app,
        post_json(
            "POST",
            "/api/config/validate",
            json!({"yaml": "screens: []\n"}),
            true,
            None,
        ),
    )
    .await;
    assert_eq!(body["ok"], false, "{body}");
    let (_, _, body) = send(&f.app, get("/api/config/yaml")).await;
    let (_, _, body) = send(
        &f.app,
        post_json(
            "POST",
            "/api/config/validate",
            json!({"yaml": body["yaml"]}),
            true,
            None,
        ),
    )
    .await;
    assert_eq!(body, json!({"ok": true, "restart": "none"}));
}
