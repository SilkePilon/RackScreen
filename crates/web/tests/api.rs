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
    /// A logged-in session (Tailscale fixtures only; "" elsewhere).
    session: String,
}

fn fixture(password: &str) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.yaml");
    let mut cfg = Config::default();
    cfg.web.password = password.into();
    cfg.github.token = "ghp_stored".into();
    cfg.save(&path).unwrap();
    let hub = Arc::new(Hub::new(path, true, "9.9.9", LogSink::new(50), 8));
    // the update job lives in the temp dir, not ~/.local/state
    let state = Arc::new(AppState::with_update_file(
        hub.clone(),
        password.into(),
        8740,
        Some(dir.path().join("state").join("update.json")),
    ));
    let app = router(state.clone());
    Fixture {
        _dir: dir,
        hub,
        state,
        app,
        session: String::new(),
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
    // with the header and no monitor running a control is unavailable, not forbidden
    let (st, _, _) = send(
        &f.app,
        post_json(
            "POST",
            "/api/control/pattern",
            json!({"on": true}),
            true,
            None,
        ),
    )
    .await;
    assert_eq!(st, StatusCode::SERVICE_UNAVAILABLE);
    // the night override is kept for the next render loop and shows at once
    let (st, _, body) = send(
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
    assert_eq!(st, StatusCode::OK);
    assert_eq!(body, json!({"ok": true, "applied": false}));
    let (_, _, body) = send(&f.app, get("/api/status")).await;
    assert_eq!(body["night"]["mode"], "on");
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
        "monitor", "night", "screens", "links", "stats", "tiles", "host", "age_ms",
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

#[tokio::test]
async fn gif_search_needs_a_key_and_keys_stay_secret() {
    let f = fixture("");
    let rx = f.hub.supervisor_mailbox();
    let (st, _, body) = send(&f.app, get("/api/gifs/providers")).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(body, json!({"giphy": false, "tenor": false}));
    // no key: 400 before any network call
    for path in [
        "/api/gifs/search?provider=giphy&q=cat",
        "/api/gifs/trending?provider=tenor",
        "/api/gifs/trending-terms?provider=giphy",
    ] {
        let (st, _, body) = send(&f.app, get(path)).await;
        assert_eq!(st, StatusCode::BAD_REQUEST, "{path}");
        assert!(
            body["error"].as_str().unwrap().contains("API key"),
            "{body}"
        );
    }
    let (st, _, _) = send(&f.app, get("/api/gifs/search?provider=imgur&q=cat")).await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    let (st, _, body) = send(&f.app, get("/api/gifs/search?provider=giphy&q=%20")).await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().unwrap().contains("q must be"));
    let long = "a".repeat(101);
    let (st, _, _) = send(
        &f.app,
        get(&format!("/api/gifs/search?provider=giphy&q={long}")),
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST);

    // set the keys: masked like every secret, saved without a restart
    let (_, _, body) = send(&f.app, get("/api/config")).await;
    assert_eq!(body["config"]["gif_search"]["giphy_key"], Value::Null);
    assert_eq!(body["secrets"]["gif_search.giphy_key"], false);
    let mut cfg = body["config"].clone();
    cfg["gif_search"]["giphy_key"] = json!("giphy-secret-key");
    cfg["gif_search"]["rating"] = json!("pg");
    let (st, _, body) = send(
        &f.app,
        post_json("PUT", "/api/config", json!({"config": cfg}), true, None),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{body}");
    assert_eq!(body["restart"], "none");
    assert_eq!(body["secrets"]["gif_search.giphy_key"], true);
    assert!(!body.to_string().contains("giphy-secret-key"));
    assert!(rx.try_recv().is_err(), "no monitor restart for search keys");
    let stored = Config::load_or_default(&f.hub.config_path).unwrap();
    assert_eq!(stored.gif_search.giphy_key, "giphy-secret-key");
    assert_eq!(stored.gif_search.rating, "pg");
    let (_, _, body) = send(&f.app, get("/api/gifs/providers")).await;
    assert_eq!(body, json!({"giphy": true, "tenor": false}));
    // a bad rating is refused
    cfg["gif_search"]["rating"] = json!("x");
    let (st, _, _) = send(
        &f.app,
        post_json("PUT", "/api/config", json!({"config": cfg}), true, None),
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    // a bad offset never reaches the provider
    let (st, _, _) = send(
        &f.app,
        get("/api/gifs/search?provider=giphy&q=cat&offset=-1"),
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn show_cards_are_cached_pngs_and_unknown_shows_404() {
    let f = fixture("");
    let resp = f
        .app
        .clone()
        .oneshot(get("/api/shows/freight/card.png?frame=2"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(resp.headers()[header::CONTENT_TYPE], "image/png");
    assert_eq!(
        resp.headers()[header::CACHE_CONTROL],
        "public, max-age=86400"
    );
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    assert!(body.starts_with(b"\x89PNG"));
    let (st, _, _) = send(&f.app, get("/api/shows/nope/card.png")).await;
    assert_eq!(st, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn system_answers_every_section_with_nulls_where_unreadable() {
    let f = fixture("");
    let (st, _, body) = send(&f.app, get("/api/system")).await;
    assert_eq!(st, StatusCode::OK);
    for key in [
        "model",
        "os",
        "kernel",
        "arch",
        "hostname",
        "uptime_secs",
        "cpu",
        "memory",
        "disk",
        "process",
        "throttled",
        "network",
    ] {
        assert!(body.get(key).is_some(), "{key} missing: {body}");
    }
    for key in ["cores", "usage_pct", "temp_c", "freq_mhz", "load"] {
        assert!(body["cpu"].get(key).is_some(), "cpu.{key} missing");
    }
    assert!(body["network"].is_array());
    assert_eq!(body["disk"]["path"], "/");
    assert!(body["process"]["uptime_secs"].is_u64());
    // the second poll right away reuses the window
    let (_, _, again) = send(&f.app, get("/api/system")).await;
    assert_eq!(again["cpu"]["usage_pct"], body["cpu"]["usage_pct"]);
}

#[tokio::test]
async fn gif_memory_joins_the_config_with_the_render_loop() {
    use rackscreen_app::config::GifCfg;
    use rackscreen_app::hub::{GifStatus, StatusSnapshot};
    use rackscreen_core::gif::{FRAME_BYTES, MAX_FRAMES};
    let f = fixture("");
    let mut cfg = Config::load_or_default(&f.hub.config_path).unwrap();
    cfg.gifs = ["cat", "dog", "spare"]
        .iter()
        .map(|n| GifCfg {
            name: n.to_string(),
            url: format!("https://example.com/{n}.gif"),
        })
        .collect();
    cfg.screens[0].roles.push("gif:cat".into());
    cfg.screens[1].roles.push("gif:dog".into());
    cfg.save(&f.hub.config_path).unwrap();
    f.hub.publish_status(StatusSnapshot {
        gifs: vec![GifStatus {
            name: "cat".into(),
            used: true,
            state: "ready",
            frames: Some(MAX_FRAMES),
            bytes: Some(MAX_FRAMES * FRAME_BYTES),
            error: None,
        }],
        ..Default::default()
    });
    let (st, _, body) = send(&f.app, get("/api/gifs/memory")).await;
    assert_eq!(st, StatusCode::OK, "{body}");
    assert_eq!(body["frame_bytes"], FRAME_BYTES);
    assert_eq!(body["max_frames"], MAX_FRAMES);
    assert_eq!(body["max_gif_mb"], 33.0);
    let gifs = body["gifs"].as_array().unwrap();
    assert_eq!(gifs.len(), 3);
    assert_eq!(gifs[0]["state"], "ready");
    assert_eq!(gifs[0]["frames"], MAX_FRAMES);
    assert_eq!(gifs[0]["mb"], 33.0);
    assert_eq!(gifs[1]["state"], "loading", "on a screen, not decoded yet");
    assert_eq!(gifs[1]["mb"], Value::Null);
    assert_eq!(gifs[2]["state"], "off");
    assert_eq!(gifs[2]["used"], false);
    assert_eq!(body["decoded_mb"], 33.0);
    assert_eq!(body["pending_mb"], 33.0);
    if let Some(avail) = body["memory"]["available_mb"].as_f64() {
        let budget = body["budget_mb"].as_f64().unwrap();
        assert!((budget - (avail + 32.96 - 150.0).max(0.0)).abs() < 0.1);
        assert!(body["fits_more"].is_u64());
    }
}

// ---------------------------------------------------------------------------
// Tailscale, through a scripted `tailscale` CLI.

use rackscreen_web::tailscale::{FakeRunner, Output as TsOut};

/// A web password is set and the requests carry a logged-in session:
/// Tailscale refuses every action without one.
fn ts_fixture(runner: Arc<FakeRunner>) -> Fixture {
    ts_fixture_pw(runner, "hunter2")
}

fn ts_fixture_pw(runner: Arc<FakeRunner>, password: &str) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.yaml");
    let mut cfg = Config::default();
    cfg.web.password = password.into();
    cfg.save(&path).unwrap();
    let hub = Arc::new(Hub::new(path, true, "9.9.9", LogSink::new(50), 8));
    let state = Arc::new(
        AppState::with_update_file(hub.clone(), password.into(), 8740, None)
            .with_tailscale(runner, "silke"),
    );
    let session = if password.is_empty() {
        String::new()
    } else {
        state.new_session()
    };
    let app = router(state.clone());
    Fixture {
        _dir: dir,
        hub,
        state,
        app,
        session,
    }
}

fn tget(f: &Fixture, path: &str) -> Request<Body> {
    let mut b = Request::get(path);
    if !f.session.is_empty() {
        b = b.header(header::COOKIE, format!("rs_session={}", f.session));
    }
    b.body(Body::empty()).unwrap()
}

fn tpost(f: &Fixture, path: &str, body: Value) -> Request<Body> {
    let cookie = (!f.session.is_empty()).then_some(f.session.as_str());
    post_json("POST", path, body, true, cookie)
}

fn running_runner() -> Arc<FakeRunner> {
    let r = Arc::new(FakeRunner::default());
    r.respond(
        "tailscale version",
        TsOut::ok("1.90.6\n  tailscale commit: abc\n"),
    );
    r.respond(
        "tailscale status --json",
        TsOut::ok(include_str!("fixtures/tailscale-status-running.json")),
    );
    r.respond(
        "tailscale debug prefs",
        TsOut::ok(include_str!("fixtures/tailscale-prefs.json")),
    );
    r.respond(
        "tailscale serve status --json",
        TsOut::ok(include_str!("fixtures/tailscale-serve-status.json")),
    );
    r
}

#[tokio::test]
async fn tailscale_missing_offers_the_setup_when_sudo_allows_it() {
    let r = Arc::new(FakeRunner::default());
    *r.missing.lock().unwrap() = true;
    r.respond("sudo -n -l", TsOut::fail("sudo: a password is required"));
    let f = ts_fixture(r.clone());
    let (st, _, body) = send(&f.app, tget(&f, "/api/tailscale")).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(body["installed"], false);
    assert_eq!(body["can_setup"], false);
    assert_eq!(body["backend_state"], Value::Null);
    assert_eq!(body["peers"], json!([]));
    assert!(r
        .calls()
        .contains(&"sudo -n -l /usr/local/bin/rackscreen tailscale-setup".to_string()));
    // the sudo answer is cached for a minute
    send(&f.app, tget(&f, "/api/tailscale")).await;
    let sudo_checks = r.calls().iter().filter(|c| c.starts_with("sudo")).count();
    assert_eq!(sudo_checks, 1);
}

#[tokio::test]
async fn tailscale_running_has_the_documented_shape() {
    let f = ts_fixture(running_runner());
    let (st, _, b) = send(&f.app, tget(&f, "/api/tailscale")).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(b["installed"], true);
    assert_eq!(b["version"], "1.90.6");
    assert_eq!(b["operator_ok"], true, "prefs name silke the operator");
    assert_eq!(b["can_setup"], true);
    assert_eq!(b["backend_state"], "Running");
    assert_eq!(b["auth_url"], Value::Null);
    assert_eq!(b["self"]["dns_name"], "rackscreen.tail1234.ts.net");
    assert_eq!(b["self"]["ips"][0], "100.101.102.103");
    assert_eq!(b["tailnet"], "silke@example.com");
    assert_eq!(b["user"]["display_name"], "Silke");
    assert_eq!(b["peers"].as_array().unwrap().len(), 3);
    assert_eq!(b["peers"][2]["online"], false);
    assert_eq!(b["ssh"], true);
    assert_eq!(
        b["serve"],
        json!({"on": true, "url": "https://rackscreen.tail1234.ts.net"})
    );
    assert_eq!(b["health"].as_array().unwrap().len(), 1);
    assert!(b["hostname"].is_string());

    // another operator: the page asks to finish the setup
    let r = running_runner();
    r.respond(
        "tailscale debug prefs",
        TsOut::ok(r#"{"OperatorUser":"pi"}"#),
    );
    let f = ts_fixture(r);
    let (_, _, b) = send(&f.app, tget(&f, "/api/tailscale")).await;
    assert_eq!(b["operator_ok"], false);
    assert_eq!(b["ssh"], false);
}

#[tokio::test]
async fn tailscale_login_validates_and_never_echoes_the_key() {
    let r = running_runner();
    let f = ts_fixture(r.clone());
    for body in [
        json!({"hostname": "Bad Name"}),
        json!({"hostname": "a;b"}),
        json!({"auth_key": "not-a-key"}),
        json!({"auth_key": "tskey-abc --ssh"}),
    ] {
        let (st, _, b) = send(&f.app, tpost(&f, "/api/tailscale/login", body.clone())).await;
        assert_eq!(st, StatusCode::BAD_REQUEST, "{body}");
        assert!(!b["error"].as_str().unwrap().contains("tskey-abc"), "{b}");
    }
    assert!(r.calls().iter().all(|c| !c.starts_with("tailscale set")));

    let (st, _, b) = send(
        &f.app,
        tpost(
            &f,
            "/api/tailscale/login",
            json!({"hostname": "rack", "auth_key": "tskey-auth-k123CNTRL-secret", "ssh": true}),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{b}");
    assert_eq!(b["running"], true);
    let calls = r.calls();
    assert!(calls.contains(&"tailscale set --hostname=rack --ssh=true".to_string()));
    assert!(calls
        .contains(&"tailscale up --auth-key=file:<tmp> --hostname=rack --ssh=true".to_string()));
    assert!(calls.iter().all(|c| !c.contains("secret")));
    // the key went through a private file, gone once `up` returned
    let seen = r.key_files.lock().unwrap().clone();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].content, "tskey-auth-k123CNTRL-secret");
    assert_eq!((seen[0].file_mode, seen[0].dir_mode), (0o600, 0o700));
    assert!(!seen[0].path.exists() && !seen[0].path.parent().unwrap().exists());

    // a rejected key reads well
    r.respond(
        "tailscale up",
        TsOut::fail("backend error: invalid key: unable to validate API key"),
    );
    let (st, _, b) = send(
        &f.app,
        tpost(
            &f,
            "/api/tailscale/login",
            json!({"auth_key": "tskey-auth-x"}),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    assert!(b["error"].as_str().unwrap().contains("auth key"), "{b}");
    let seen = r.key_files.lock().unwrap().clone();
    assert!(!seen[1].path.exists(), "removed after a failure too");

    // an error that echoes the key never shows it
    r.respond(
        "tailscale up",
        TsOut::fail("weird: tskey-auth-k123CNTRL-secret rejected"),
    );
    let (_, _, b) = send(
        &f.app,
        tpost(
            &f,
            "/api/tailscale/login",
            json!({"auth_key": "tskey-auth-k123CNTRL-secret"}),
        ),
    )
    .await;
    let e = b["error"].as_str().unwrap();
    assert!(!e.contains("secret") && e.contains("tskey-***"), "{b}");

    // a Tailscale too old for `file:` gets it on the command line
    r.respond("tailscale version", TsOut::ok("1.20.4\n"));
    r.respond("tailscale up", TsOut::ok(""));
    send(
        &f.app,
        tpost(
            &f,
            "/api/tailscale/login",
            json!({"auth_key": "tskey-auth-x"}),
        ),
    )
    .await;
    assert_eq!(
        r.calls().last().unwrap(),
        "tailscale up --auth-key=*** --hostname=rackscreen --ssh=true"
    );
}

#[tokio::test]
async fn tailscale_browser_login_returns_the_link() {
    let r = running_runner();
    r.respond(
        "tailscale status --json",
        TsOut::ok(r#"{"BackendState":"NeedsLogin","AuthURL":""}"#),
    );
    *r.spawn_output.lock().unwrap() =
        "\nTo authenticate, visit:\n\n\thttps://login.tailscale.com/a/abc123\n".into();
    let f = ts_fixture(r.clone());
    let (st, _, b) = send(&f.app, tpost(&f, "/api/tailscale/login", json!({}))).await;
    assert_eq!(st, StatusCode::OK, "{b}");
    assert_eq!(b["auth_url"], "https://login.tailscale.com/a/abc123");
    assert!(r.calls().contains(&"tailscale up".to_string()));
    assert!(
        r.calls().iter().all(|c| !c.starts_with("tailscale set")),
        "nothing to set"
    );

    // or from the status, when the CLI has not printed it yet
    *r.spawn_output.lock().unwrap() = String::new();
    r.respond(
        "tailscale status --json",
        TsOut::ok(include_str!("fixtures/tailscale-status-needslogin.json")),
    );
    let (_, _, b) = send(
        &f.app,
        tpost(&f, "/api/tailscale/login", json!({"ssh": false})),
    )
    .await;
    assert_eq!(b["auth_url"], "https://login.tailscale.com/a/1a2b3c4d5e6f");
}

#[tokio::test]
async fn tailscale_actions_map_to_fixed_commands_and_readable_errors() {
    let r = running_runner();
    let f = ts_fixture(r.clone());
    for (path, body, call) in [
        ("/api/tailscale/up", json!({}), "tailscale up"),
        ("/api/tailscale/down", json!({}), "tailscale down"),
        ("/api/tailscale/logout", json!({}), "tailscale logout"),
        (
            "/api/tailscale/ssh",
            json!({"on": true}),
            "tailscale set --ssh=true",
        ),
        (
            "/api/tailscale/ssh",
            json!({"on": false}),
            "tailscale set --ssh=false --accept-risk=lose-ssh",
        ),
        (
            "/api/tailscale/serve",
            json!({"on": false}),
            "tailscale serve --https=443 off",
        ),
    ] {
        let (st, _, b) = send(&f.app, tpost(&f, path, body)).await;
        assert_eq!(st, StatusCode::OK, "{path}: {b}");
        assert_eq!(r.calls().last().unwrap(), call);
    }
    // already shared: nothing to run
    let n = r.calls().len();
    let (st, _, b) = send(
        &f.app,
        tpost(&f, "/api/tailscale/serve", json!({"on": true})),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{b}");
    assert_eq!(b["url"], "https://rackscreen.tail1234.ts.net");
    assert_eq!(r.calls()[n..], ["tailscale serve status --json"]);

    // nothing served yet: add ours, never with --yes
    r.respond("tailscale serve status --json", TsOut::ok("{}"));
    r.respond(
        "tailscale serve --bg",
        TsOut::ok("Available within your tailnet:\n\nhttps://rackscreen.tail1234.ts.net/\n|-- proxy http://127.0.0.1:8740\n"),
    );
    let (st, _, b) = send(
        &f.app,
        tpost(&f, "/api/tailscale/serve", json!({"on": true})),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{b}");
    assert_eq!(b["url"], "https://rackscreen.tail1234.ts.net");
    assert!(r
        .calls()
        .contains(&"tailscale serve --bg --https=443 http://127.0.0.1:8740".to_string()));
    // and turning off what is not there runs nothing
    let n = r.calls().len();
    let (st, _, b) = send(
        &f.app,
        tpost(&f, "/api/tailscale/serve", json!({"on": false})),
    )
    .await;
    assert_eq!((st, &b["on"]), (StatusCode::OK, &json!(false)));
    assert_eq!(r.calls()[n..], ["tailscale serve status --json"]);

    // HTTPS not enabled on the tailnet: the link to turn it on
    r.respond(
        "tailscale serve --bg",
        TsOut {
            status: None,
            stdout: "Serve is not enabled on your tailnet.\nTo enable, visit:\n\n   https://login.tailscale.com/f/serve?node=nX\n".into(),
            timed_out: true,
            ..TsOut::default()
        },
    );
    let (st, _, b) = send(
        &f.app,
        tpost(&f, "/api/tailscale/serve", json!({"on": true})),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(b["on"], false);
    assert_eq!(
        b["enable_url"],
        "https://login.tailscale.com/f/serve?node=nX"
    );

    // not the operator
    r.respond(
        "tailscale down",
        TsOut::fail("Access denied: prefs write access denied\n\nUse 'sudo tailscale down'.\nTo not require root, use 'sudo tailscale set --operator=$USER' once."),
    );
    let (st, _, b) = send(&f.app, tpost(&f, "/api/tailscale/down", json!({}))).await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    assert!(
        b["error"].as_str().unwrap().contains("finish the setup"),
        "{b}"
    );

    // no CSRF header, no command
    let n = r.calls().len();
    let (st, _, _) = send(
        &f.app,
        post_json(
            "POST",
            "/api/tailscale/logout",
            json!({}),
            false,
            Some(&f.session),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    assert_eq!(r.calls().len(), n);
}

#[tokio::test]
async fn tailscale_setup_runs_the_root_helper_as_a_job() {
    let r = Arc::new(FakeRunner::default());
    *r.stream_lines.lock().unwrap() = (
        vec![
            "Installing Tailscale with the official installer".into(),
            "silke may now manage Tailscale without sudo".into(),
        ],
        true,
    );
    let f = ts_fixture(r.clone());
    let (_, _, b) = send(&f.app, tget(&f, "/api/tailscale/setup")).await;
    assert_eq!(b, json!({"state": "idle", "log": []}));
    let (st, _, _) = send(&f.app, tpost(&f, "/api/tailscale/setup", json!({}))).await;
    assert_eq!(st, StatusCode::OK);
    let mut job = Value::Null;
    for _ in 0..50 {
        job = send(&f.app, tget(&f, "/api/tailscale/setup")).await.2;
        if job["state"] != "running" {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert_eq!(job["state"], "done", "{job}");
    assert_eq!(job["log"].as_array().unwrap().len(), 3);
    assert!(r
        .calls()
        .contains(&"sudo -n /usr/local/bin/rackscreen tailscale-setup".to_string()));

    // sudo says no: the job says how to allow it
    *r.stream_lines.lock().unwrap() = (vec!["sudo: a password is required".into()], false);
    send(&f.app, tpost(&f, "/api/tailscale/setup", json!({}))).await;
    for _ in 0..50 {
        job = send(&f.app, tget(&f, "/api/tailscale/setup")).await.2;
        if job["state"] != "running" {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert_eq!(job["state"], "failed");
    assert!(job["log"]
        .as_array()
        .unwrap()
        .last()
        .unwrap()
        .as_str()
        .unwrap()
        .contains("sudo rackscreen install"));
}

#[tokio::test]
async fn tailscale_actions_need_a_web_password() {
    let r = running_runner();
    let f = ts_fixture_pw(r.clone(), "");
    // reading is fine, and says why the actions are off
    let (st, _, b) = send(&f.app, tget(&f, "/api/tailscale")).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(b["password_set"], false);
    let before = r.calls().len();
    for (path, body) in [
        ("/api/tailscale/setup", json!({})),
        ("/api/tailscale/login", json!({})),
        ("/api/tailscale/login", json!({"auth_key": "tskey-auth-x"})),
        ("/api/tailscale/up", json!({})),
        ("/api/tailscale/down", json!({})),
        ("/api/tailscale/logout", json!({})),
        ("/api/tailscale/ssh", json!({"on": true})),
        ("/api/tailscale/serve", json!({"on": true})),
        ("/api/tailscale/serve", json!({"on": false})),
    ] {
        let (st, _, b) = send(&f.app, tpost(&f, path, body)).await;
        assert_eq!(st, StatusCode::FORBIDDEN, "{path}: {b}");
        assert_eq!(
            b["error"],
            "Set a web password first (System → Web access): Tailscale can give remote access to this Pi."
        );
    }
    assert_eq!(r.calls().len(), before, "no command ran");

    // with a password, the guard wants the session
    let f = ts_fixture(r.clone());
    let (_, _, b) = send(&f.app, tget(&f, "/api/tailscale")).await;
    assert_eq!(b["password_set"], true);
    let (st, _, _) = send(
        &f.app,
        post_json("POST", "/api/tailscale/up", json!({}), true, None),
    )
    .await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_waiting_browser_login_expires_but_never_kills_a_newer_one() {
    let r = running_runner();
    r.respond(
        "tailscale status --json",
        TsOut::ok(r#"{"BackendState":"NeedsLogin","AuthURL":""}"#),
    );
    *r.spawn_output.lock().unwrap() = "https://login.tailscale.com/a/abc123\n".into();
    let f = ts_fixture(r.clone());
    *f.state.tailscale.login_ttl.lock().unwrap() = std::time::Duration::from_millis(300);
    let sleep = |ms| tokio::time::sleep(std::time::Duration::from_millis(ms));

    let (st, _, _) = send(&f.app, tpost(&f, "/api/tailscale/login", json!({}))).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(r.spawns_alive(), [true]);
    sleep(150).await;
    send(&f.app, tpost(&f, "/api/tailscale/login", json!({}))).await;
    assert_eq!(
        r.spawns_alive(),
        [false, true],
        "the new login replaced the old"
    );
    // the first login's timer fires now: it must leave the second alone
    sleep(250).await;
    assert_eq!(r.spawns_alive(), [false, true]);
    // and the second one's own timer ends it
    sleep(200).await;
    assert_eq!(r.spawns_alive(), [false, false]);

    // two logins at once both answer (one after the other)
    let (a, b) = tokio::join!(
        send(&f.app, tpost(&f, "/api/tailscale/login", json!({}))),
        send(&f.app, tpost(&f, "/api/tailscale/login", json!({}))),
    );
    assert_eq!((a.0, b.0), (StatusCode::OK, StatusCode::OK));
    assert_eq!(r.spawns_alive()[2..], [false, true]);
}

#[tokio::test]
async fn tailscale_serve_leaves_someone_elses_https_443_alone() {
    let r = running_runner();
    r.respond(
        "tailscale serve status --json",
        TsOut::ok(r#"{"Web":{"rackscreen.tail1234.ts.net:443":{"Handlers":{"/":{"Proxy":"http://127.0.0.1:3000"}}}}}"#),
    );
    let f = ts_fixture(r.clone());
    let (_, _, b) = send(&f.app, tget(&f, "/api/tailscale")).await;
    assert_eq!(b["serve"]["on"], false);
    for on in [true, false] {
        let (st, _, b) = send(
            &f.app,
            tpost(&f, "/api/tailscale/serve", json!({ "on": on })),
        )
        .await;
        assert_eq!(st, StatusCode::CONFLICT, "{b}");
        let e = b["error"].as_str().unwrap();
        assert!(
            e.contains("https:443") && e.contains("http://127.0.0.1:3000"),
            "{e}"
        );
    }
    assert!(
        r.calls()
            .iter()
            .all(|c| !c.starts_with("tailscale serve --")),
        "{:?}",
        r.calls()
    );

    // an unreadable status changes nothing either
    r.respond("tailscale serve status --json", TsOut::fail("boom"));
    let (st, _, _) = send(
        &f.app,
        tpost(&f, "/api/tailscale/serve", json!({ "on": false })),
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    assert!(r
        .calls()
        .iter()
        .all(|c| !c.starts_with("tailscale serve --")));
}
