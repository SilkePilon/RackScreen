//! A Bambu Lab account instead of LAN only mode: the login, the account's
//! printers and the token file the printer source reads.
//!
//! None of this is a published API. It is what Bambu Studio and the slicers
//! use, as worked out by ha-bambulab, openHAB and Bambu-Lab-Cloud-API:
//!
//! - `POST <api>/v1/user-service/user/login {account, password}` gives a
//!   token, or asks for an emailed code (`loginType: verifyCode`) or an
//!   authenticator code (`loginType: tfa`, with a `tfaKey`).
//! - The emailed code is sent by `.../user/sendemail/code` and posted back
//!   to the login as `{account, code}`.
//! - The authenticator code goes to `<web>/api/sign-in/tfa` with a CSRF
//!   cookie from `<web>/api/csrf`; the token comes back as a cookie.
//! - The token is a JWT whose `username` is the MQTT user, `u_<uid>`; it
//!   lasts about three months and cannot be refreshed.
//! - `<api>/v1/iot-service/api/user/bind` lists the account's printers.
//!
//! The password only passes through `login`; the token file keeps the
//! token, the MQTT user and the account name.

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Which of Bambu's clouds the account lives in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Region {
    #[default]
    Global,
    China,
}

impl Region {
    pub fn mqtt_host(self) -> &'static str {
        match self {
            Region::Global => "us.mqtt.bambulab.com",
            Region::China => "cn.mqtt.bambulab.com",
        }
    }
}

/// The two hosts the login talks to; tests point both at a local server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Endpoints {
    /// `https://api.bambulab.com`
    pub api: String,
    /// `https://bambulab.com`, for the authenticator code.
    pub web: String,
}

impl Endpoints {
    pub fn for_region(r: Region) -> Self {
        match r {
            Region::Global => Self {
                api: "https://api.bambulab.com".into(),
                web: "https://bambulab.com".into(),
            },
            Region::China => Self {
                api: "https://api.bambulab.cn".into(),
                web: "https://bambulab.cn".into(),
            },
        }
    }
}

/// Without a readable expiry the token is taken to last this long.
pub const DEFAULT_LIFETIME_SECS: i64 = 90 * 24 * 3600;

/// What the login needs next.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoginStep {
    Done(String),
    /// A code was (or is about to be) emailed to the account.
    EmailCode,
    /// An authenticator app code, sent with this key.
    Tfa {
        tfa_key: String,
    },
}

/// A login that failed, in words the web UI can show as they are.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CloudError {
    #[error(
        "Bambu Lab blocked the login from here (it sometimes does); try again in a few minutes"
    )]
    Blocked,
    #[error("that code is not right")]
    CodeWrong,
    #[error("that code expired; a new one is on its way")]
    CodeExpired,
    #[error("the login was refused: {0}")]
    Refused(String),
    #[error("Bambu Lab sent an answer this does not understand: {0}")]
    Unexpected(String),
}

/// The message in a Bambu error body, if any.
fn message(v: &Value) -> Option<String> {
    ["error", "message", "msg"]
        .iter()
        .filter_map(|k| v.get(*k).and_then(Value::as_str))
        .map(str::trim)
        .find(|s| !s.is_empty() && *s != "success")
        .map(str::to_string)
}

fn parse_json(status: u16, body: &str) -> Result<Value, CloudError> {
    match serde_json::from_str::<Value>(body) {
        Ok(v) => Ok(v),
        // Cloudflare's challenge page, or another HTML error
        Err(_) if status == 403 || status == 429 || status == 503 => Err(CloudError::Blocked),
        Err(_) => Err(CloudError::Unexpected(format!(
            "HTTP {status}, {}",
            body.chars().take(80).collect::<String>()
        ))),
    }
}

/// The answer to the password login.
pub fn parse_login(status: u16, body: &str) -> Result<LoginStep, CloudError> {
    let v = parse_json(status, body)?;
    if let Some(t) = v.get("accessToken").and_then(Value::as_str) {
        if !t.trim().is_empty() {
            return Ok(LoginStep::Done(t.trim().to_string()));
        }
    }
    match v.get("loginType").and_then(Value::as_str) {
        Some("verifyCode") => return Ok(LoginStep::EmailCode),
        Some("tfa") => {
            let key = v.get("tfaKey").and_then(Value::as_str).unwrap_or("");
            if key.is_empty() {
                return Err(CloudError::Unexpected(
                    "a 2FA login without a tfaKey".into(),
                ));
            }
            return Ok(LoginStep::Tfa {
                tfa_key: key.to_string(),
            });
        }
        _ => {}
    }
    if !(200..300).contains(&status) || message(&v).is_some() {
        return Err(CloudError::Refused(
            message(&v).unwrap_or_else(|| format!("HTTP {status}")),
        ));
    }
    Err(CloudError::Unexpected("no token and no next step".into()))
}

/// The answer to the emailed code: `code` 1 in a 400 is an expired code,
/// 2 a wrong one.
pub fn parse_code(status: u16, body: &str) -> Result<String, CloudError> {
    let v = parse_json(status, body)?;
    if status == 400 {
        return Err(match v.get("code").and_then(Value::as_i64) {
            Some(1) => CloudError::CodeExpired,
            Some(2) => CloudError::CodeWrong,
            _ => CloudError::Refused(message(&v).unwrap_or_else(|| "HTTP 400".into())),
        });
    }
    match parse_login(status, body)? {
        LoginStep::Done(t) => Ok(t),
        _ => Err(CloudError::CodeWrong),
    }
}

/// The `name=value` of cookie `name` among `Set-Cookie` header values.
pub fn cookie<'a>(set_cookies: impl IntoIterator<Item = &'a str>, name: &str) -> Option<String> {
    set_cookies.into_iter().find_map(|c| {
        let first = c.split(';').next()?.trim();
        let (k, v) = first.split_once('=')?;
        (k.trim() == name && !v.trim().is_empty()).then(|| v.trim().to_string())
    })
}

/// The JWT's payload, if the token is one.
fn jwt_payload(token: &str) -> Option<Value> {
    let mut parts = token.split('.');
    let (_, payload, _) = (parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some() {
        return None;
    }
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload.trim_end_matches('='))
        .ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// `u_<uid>` from a JWT token.
pub fn username_from_token(token: &str) -> Option<String> {
    let u = jwt_payload(token)?
        .get("username")?
        .as_str()?
        .trim()
        .to_string();
    (!u.is_empty()).then_some(u)
}

/// The token's `exp`, in unix seconds.
pub fn expiry_from_token(token: &str) -> Option<i64> {
    jwt_payload(token)?.get("exp")?.as_i64()
}

/// A printer bound to the account.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CloudPrinter {
    pub serial: String,
    pub name: String,
    pub model: String,
    pub online: bool,
}

/// The bind list.
pub fn parse_printers(status: u16, body: &str) -> Result<Vec<CloudPrinter>, CloudError> {
    let v = parse_json(status, body)?;
    if status == 401 || status == 403 {
        return Err(CloudError::Refused("the login expired".into()));
    }
    let Some(devices) = v.get("devices").and_then(Value::as_array) else {
        return Err(CloudError::Unexpected(
            message(&v).unwrap_or_else(|| format!("HTTP {status}, no printer list")),
        ));
    };
    let s = |d: &Value, k: &str| d.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    Ok(devices
        .iter()
        .filter_map(|d| {
            let serial = s(d, "dev_id");
            if serial.is_empty() {
                return None;
            }
            let model = Some(s(d, "dev_product_name"))
                .filter(|m| !m.is_empty())
                .unwrap_or_else(|| s(d, "dev_model_name"));
            Some(CloudPrinter {
                name: Some(s(d, "name"))
                    .filter(|n| !n.is_empty())
                    .unwrap_or_else(|| serial.clone()),
                serial,
                model,
                online: d.get("online").and_then(Value::as_bool).unwrap_or(false),
            })
        })
        .collect())
}

// ---- the token file --------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenFile {
    pub token: String,
    /// The MQTT user, `u_<uid>`.
    pub username: String,
    /// The email the login was made with, for the status line.
    pub account: String,
    pub region: Region,
    /// Unix seconds.
    pub expires_at: i64,
    /// Set when the broker refused the token: log in again.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl TokenFile {
    /// Logged in, as far as anyone knows.
    pub fn usable(&self, now: i64) -> bool {
        self.error.is_none() && self.expires_at > now && !self.token.is_empty()
    }
}

pub fn token_path_in(state_dir: &Path) -> PathBuf {
    state_dir.join("rackscreen").join("bambu.json")
}

pub fn read_token(path: &Path) -> Option<TokenFile> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

/// Mode 0600 in a 0700 directory, via a temp file and a rename.
pub fn write_token(path: &Path, tok: &TokenFile, owner: Option<(u32, u32)>) -> Result<()> {
    use crate::owned_fs::{create_dir_all_owned_mode, write_new_owned};
    let dir = path.parent().context("token path has no parent")?;
    create_dir_all_owned_mode(dir, 0o700, owner)?;
    write_new_owned(
        path,
        &path.with_extension("json.tmp"),
        serde_json::to_string_pretty(tok)?.as_bytes(),
        0o600,
        owner,
    )
}

/// The broker refused the token: keep the file (the UI still knows who was
/// logged in) but say so.
pub fn mark_refused(path: &Path) -> Result<()> {
    let Some(mut tok) = read_token(path) else {
        return Ok(());
    };
    if tok.error.is_none() {
        tok.error = Some("refused by Bambu Lab".into());
        write_token(path, &tok, None)?;
    }
    Ok(())
}

/// `logged in as x@y`, for the web UI.
pub fn login_status(tok: Option<&TokenFile>, now: i64) -> String {
    match tok {
        None => "not logged in".into(),
        Some(t) if t.error.is_some() => "the login was refused, log in again".into(),
        Some(t) if t.expires_at <= now => "the login expired, log in again".into(),
        Some(t) => format!("logged in as {}", t.account),
    }
}

// ---- the requests ----------------------------------------------------------

/// Bambu's own client for these calls: JSON, a longer timeout than the
/// shared one, no redirects (an answer elsewhere is not a login).
pub fn client() -> reqwest::Client {
    let _ = rustls::crypto::ring::default_provider().install_default();
    reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(concat!("rackscreen/", env!("CARGO_PKG_VERSION")))
        .build()
        .expect("reqwest client")
}

fn reach(e: reqwest::Error) -> anyhow::Error {
    anyhow::anyhow!("could not reach Bambu Lab: {e}")
}

async fn post(http: &reqwest::Client, url: &str, body: Value) -> Result<(u16, String)> {
    let resp = http
        .post(url)
        .header("Accept", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(reach)?;
    let status = resp.status().as_u16();
    Ok((status, resp.text().await.unwrap_or_default()))
}

/// The password step. An emailed code is requested right away when the
/// account needs one.
pub async fn login(
    http: &reqwest::Client,
    ep: &Endpoints,
    account: &str,
    password: &str,
) -> Result<LoginStep> {
    let (status, body) = post(
        http,
        &format!("{}/v1/user-service/user/login", ep.api),
        json!({ "account": account, "password": password, "apiError": "" }),
    )
    .await?;
    let step = parse_login(status, &body)?;
    if step == LoginStep::EmailCode {
        send_code(http, ep, account).await?;
    }
    Ok(step)
}

/// Email a (new) login code.
pub async fn send_code(http: &reqwest::Client, ep: &Endpoints, account: &str) -> Result<()> {
    let (status, body) = post(
        http,
        &format!("{}/v1/user-service/user/sendemail/code", ep.api),
        json!({ "email": account, "type": "codeLogin" }),
    )
    .await?;
    if (200..300).contains(&status) {
        return Ok(());
    }
    let v = parse_json(status, &body)?;
    Err(CloudError::Refused(message(&v).unwrap_or_else(|| format!("HTTP {status}"))).into())
}

/// The emailed code; an expired one has a new one sent.
pub async fn submit_code(
    http: &reqwest::Client,
    ep: &Endpoints,
    account: &str,
    code: &str,
) -> Result<String> {
    let (status, body) = post(
        http,
        &format!("{}/v1/user-service/user/login", ep.api),
        json!({ "account": account, "code": code }),
    )
    .await?;
    match parse_code(status, &body) {
        Err(CloudError::CodeExpired) => {
            send_code(http, ep, account).await?;
            Err(CloudError::CodeExpired.into())
        }
        r => Ok(r?),
    }
}

/// The authenticator code.
pub async fn submit_tfa(
    http: &reqwest::Client,
    ep: &Endpoints,
    tfa_key: &str,
    code: &str,
) -> Result<String> {
    let resp = http
        .get(format!("{}/api/csrf", ep.web))
        .send()
        .await
        .map_err(reach)?;
    let set: Vec<String> = resp
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok().map(str::to_string))
        .collect();
    let Some(csrf) = cookie(set.iter().map(String::as_str), "bbl_csrf_token") else {
        if resp.status().as_u16() == 403 {
            return Err(CloudError::Blocked.into());
        }
        return Err(CloudError::Unexpected("no CSRF cookie for the 2FA step".into()).into());
    };
    let resp = http
        .post(format!("{}/api/sign-in/tfa", ep.web))
        .header("x-bbl-csrf-token", &csrf)
        .header(reqwest::header::COOKIE, format!("bbl_csrf_token={csrf}"))
        .json(&json!({ "tfaKey": tfa_key, "tfaCode": code }))
        .send()
        .await
        .map_err(reach)?;
    let status = resp.status().as_u16();
    let set: Vec<String> = resp
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok().map(str::to_string))
        .collect();
    if let Some(t) = cookie(set.iter().map(String::as_str), "token") {
        return Ok(t);
    }
    let body = resp.text().await.unwrap_or_default();
    let v = parse_json(status, &body)?;
    if (400..500).contains(&status) {
        return Err(CloudError::CodeWrong.into());
    }
    Err(CloudError::Unexpected(message(&v).unwrap_or_else(|| format!("HTTP {status}"))).into())
}

/// The MQTT user: from the JWT, else the account's `uid`.
pub async fn username(http: &reqwest::Client, ep: &Endpoints, token: &str) -> Result<String> {
    if let Some(u) = username_from_token(token) {
        return Ok(u);
    }
    let resp = http
        .get(format!("{}/v1/design-user-service/my/preference", ep.api))
        .bearer_auth(token)
        .send()
        .await
        .map_err(reach)?;
    let status = resp.status().as_u16();
    let v = parse_json(status, &resp.text().await.unwrap_or_default())?;
    let uid = match v.get("uid") {
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::String(s)) if !s.trim().is_empty() => s.trim().to_string(),
        _ => return Err(CloudError::Unexpected("no uid for the account".into()).into()),
    };
    Ok(format!("u_{uid}"))
}

/// The account's printers.
pub async fn printers(
    http: &reqwest::Client,
    ep: &Endpoints,
    token: &str,
) -> Result<Vec<CloudPrinter>> {
    let resp = http
        .get(format!("{}/v1/iot-service/api/user/bind", ep.api))
        .bearer_auth(token)
        .send()
        .await
        .map_err(reach)?;
    let status = resp.status().as_u16();
    Ok(parse_printers(
        status,
        &resp.text().await.unwrap_or_default(),
    )?)
}

/// The token file for a fresh token.
pub async fn finish(
    http: &reqwest::Client,
    ep: &Endpoints,
    region: Region,
    account: &str,
    token: String,
    now: i64,
) -> Result<TokenFile> {
    let username = username(http, ep, &token).await?;
    let expires_at = expiry_from_token(&token)
        .filter(|e| *e > now)
        .unwrap_or(now + DEFAULT_LIFETIME_SECS);
    Ok(TokenFile {
        token,
        username,
        account: account.trim().to_string(),
        region,
        expires_at,
        error: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    fn jwt(payload: Value) -> String {
        let e = |b: &[u8]| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b);
        format!(
            "{}.{}.{}",
            e(br#"{"alg":"none"}"#),
            e(payload.to_string().as_bytes()),
            e(b"sig")
        )
    }

    #[test]
    fn login_answers() {
        assert_eq!(
            parse_login(200, r#"{"accessToken":"abc","loginType":""}"#),
            Ok(LoginStep::Done("abc".into()))
        );
        assert_eq!(
            parse_login(200, r#"{"accessToken":"","loginType":"verifyCode"}"#),
            Ok(LoginStep::EmailCode)
        );
        assert_eq!(
            parse_login(200, r#"{"accessToken":"","loginType":"tfa","tfaKey":"k1"}"#),
            Ok(LoginStep::Tfa {
                tfa_key: "k1".into()
            })
        );
        assert_eq!(
            parse_login(400, r#"{"code":3,"error":"Incorrect password"}"#),
            Err(CloudError::Refused("Incorrect password".into()))
        );
        assert_eq!(
            parse_login(403, "<html>Just a moment...</html>"),
            Err(CloudError::Blocked)
        );
        assert!(matches!(
            parse_login(500, "oops"),
            Err(CloudError::Unexpected(_))
        ));
        assert!(matches!(
            parse_login(200, r#"{"loginType":"tfa"}"#),
            Err(CloudError::Unexpected(_))
        ));
    }

    #[test]
    fn code_answers() {
        assert_eq!(
            parse_code(200, r#"{"accessToken":"t"}"#),
            Ok("t".to_string())
        );
        assert_eq!(
            parse_code(400, r#"{"code":1}"#),
            Err(CloudError::CodeExpired)
        );
        assert_eq!(parse_code(400, r#"{"code":2}"#), Err(CloudError::CodeWrong));
        assert_eq!(
            parse_code(200, r#"{"accessToken":"","loginType":"verifyCode"}"#),
            Err(CloudError::CodeWrong)
        );
    }

    #[test]
    fn jwt_username_and_expiry() {
        let t = jwt(json!({"username": "u_123456", "exp": 1_800_000_000}));
        assert_eq!(username_from_token(&t).as_deref(), Some("u_123456"));
        assert_eq!(expiry_from_token(&t), Some(1_800_000_000));
        assert_eq!(username_from_token("not-a-jwt"), None);
        assert_eq!(username_from_token("a.b.c.d"), None);
        assert_eq!(username_from_token(&jwt(json!({"username": ""}))), None);
    }

    #[test]
    fn cookies_from_set_cookie() {
        let set = [
            "a=1; Path=/",
            "bbl_csrf_token=xyz; Path=/; HttpOnly",
            "token=; Max-Age=0",
        ];
        assert_eq!(cookie(set, "bbl_csrf_token").as_deref(), Some("xyz"));
        assert_eq!(cookie(set, "token"), None, "an emptied cookie is no token");
        assert_eq!(cookie(set, "missing"), None);
    }

    #[test]
    fn printer_list() {
        let body = r#"{"message":"success","code":null,"devices":[
            {"dev_id":"01P00A1","name":"Workshop","online":true,"dev_product_name":"P1S","dev_model_name":"C12"},
            {"dev_id":"03W00X2","name":"","online":false,"dev_model_name":"N2S"},
            {"name":"no serial"}
        ]}"#;
        let p = parse_printers(200, body).unwrap();
        assert_eq!(p.len(), 2);
        assert_eq!(
            p[0],
            CloudPrinter {
                serial: "01P00A1".into(),
                name: "Workshop".into(),
                model: "P1S".into(),
                online: true
            }
        );
        assert_eq!(
            (p[1].name.as_str(), p[1].model.as_str()),
            ("03W00X2", "N2S")
        );
        assert!(parse_printers(401, r#"{"message":"expired"}"#).is_err());
    }

    #[test]
    fn token_file_round_trip_and_refusal() {
        let dir = tempfile::tempdir().unwrap();
        let path = token_path_in(dir.path());
        assert_eq!(path, dir.path().join("rackscreen/bambu.json"));
        assert_eq!(read_token(&path), None);
        let tok = TokenFile {
            token: "t".into(),
            username: "u_1".into(),
            account: "me@example.com".into(),
            region: Region::Global,
            expires_at: 2_000,
            error: None,
        };
        write_token(&path, &tok, None).unwrap();
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(read_token(&path).as_ref(), Some(&tok));
        assert!(tok.usable(1_000) && !tok.usable(2_000));
        assert_eq!(
            login_status(Some(&tok), 1_000),
            "logged in as me@example.com"
        );
        assert_eq!(
            login_status(Some(&tok), 3_000),
            "the login expired, log in again"
        );
        mark_refused(&path).unwrap();
        let t = read_token(&path).unwrap();
        assert!(t.error.is_some() && !t.usable(1_000));
        assert_eq!(
            login_status(Some(&t), 1_000),
            "the login was refused, log in again"
        );
        assert_eq!(login_status(None, 0), "not logged in");
    }

    /// A one-request-per-connection HTTP server that answers by path and
    /// keeps the request bodies it saw.
    async fn serve(
        answers: Vec<(&'static str, u16, String, Vec<&'static str>)>,
    ) -> (
        String,
        std::sync::Arc<std::sync::Mutex<Vec<(String, String)>>>,
    ) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let seen2 = seen.clone();
        tokio::spawn(async move {
            let mut answers = answers;
            loop {
                let Ok((mut s, _)) = listener.accept().await else {
                    return;
                };
                let mut buf = Vec::new();
                let mut chunk = [0u8; 4096];
                let (head, body) = loop {
                    let n = s.read(&mut chunk).await.unwrap();
                    if n == 0 {
                        break (String::new(), String::new());
                    }
                    buf.extend_from_slice(&chunk[..n]);
                    let text = String::from_utf8_lossy(&buf).to_string();
                    if let Some((h, b)) = text.split_once("\r\n\r\n") {
                        let len = h
                            .lines()
                            .find_map(|l| {
                                l.to_ascii_lowercase()
                                    .strip_prefix("content-length:")
                                    .map(|v| v.trim().parse::<usize>().unwrap())
                            })
                            .unwrap_or(0);
                        if b.len() >= len {
                            break (h.to_string(), b.to_string());
                        }
                    }
                };
                let path = head.split_whitespace().nth(1).unwrap_or("").to_string();
                seen2.lock().unwrap().push((path.clone(), body));
                let i = answers
                    .iter()
                    .position(|(p, ..)| path.starts_with(p))
                    .expect("an answer for the path");
                let (_, status, body, cookies) = answers.remove(i);
                let mut resp = format!(
                    "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n",
                    body.len()
                );
                for c in cookies {
                    resp += &format!("Set-Cookie: {c}\r\n");
                }
                resp += "\r\n";
                resp += &body;
                s.write_all(resp.as_bytes()).await.unwrap();
            }
        });
        (base, seen)
    }

    #[tokio::test]
    async fn email_code_login_then_printers() {
        let token = jwt(json!({"username": "u_42", "exp": 1_900_000_000}));
        let (base, seen) = serve(vec![
            (
                "/v1/user-service/user/login",
                200,
                r#"{"accessToken":"","loginType":"verifyCode"}"#.into(),
                vec![],
            ),
            (
                "/v1/user-service/user/sendemail/code",
                200,
                "{}".into(),
                vec![],
            ),
            (
                "/v1/user-service/user/login",
                400,
                r#"{"code":2}"#.into(),
                vec![],
            ),
            (
                "/v1/user-service/user/login",
                200,
                format!(r#"{{"accessToken":"{token}"}}"#),
                vec![],
            ),
            (
                "/v1/iot-service/api/user/bind",
                200,
                r#"{"devices":[{"dev_id":"S1","name":"P","online":true}]}"#.into(),
                vec![],
            ),
        ])
        .await;
        let ep = Endpoints {
            api: base.clone(),
            web: base,
        };
        let http = client();
        assert_eq!(
            login(&http, &ep, "me@example.com", "pw").await.unwrap(),
            LoginStep::EmailCode
        );
        let e = submit_code(&http, &ep, "me@example.com", "000000")
            .await
            .unwrap_err();
        assert_eq!(e.downcast_ref::<CloudError>(), Some(&CloudError::CodeWrong));
        let t = submit_code(&http, &ep, "me@example.com", "123456")
            .await
            .unwrap();
        let file = finish(&http, &ep, Region::Global, " me@example.com ", t, 1_000)
            .await
            .unwrap();
        assert_eq!(file.username, "u_42");
        assert_eq!(file.expires_at, 1_900_000_000);
        assert_eq!(file.account, "me@example.com");
        let p = printers(&http, &ep, &file.token).await.unwrap();
        assert_eq!(p[0].serial, "S1");
        let seen = seen.lock().unwrap();
        assert!(seen[0].1.contains(r#""password":"pw""#));
        assert!(seen[1].1.contains(r#""type":"codeLogin""#));
        assert!(seen[3].1.contains(r#""code":"123456""#));
    }

    #[tokio::test]
    async fn tfa_login_reads_the_token_cookie() {
        let (base, seen) = serve(vec![
            (
                "/api/csrf",
                204,
                String::new(),
                vec!["bbl_csrf_token=c1; Path=/"],
            ),
            (
                "/api/sign-in/tfa",
                200,
                "{}".into(),
                vec!["token=tok; Path=/; HttpOnly"],
            ),
            (
                "/v1/design-user-service/my/preference",
                200,
                r#"{"uid":77}"#.into(),
                vec![],
            ),
        ])
        .await;
        let ep = Endpoints {
            api: base.clone(),
            web: base,
        };
        let http = client();
        let t = submit_tfa(&http, &ep, "k1", "654321").await.unwrap();
        assert_eq!(t, "tok");
        assert_eq!(username(&http, &ep, &t).await.unwrap(), "u_77");
        let seen = seen.lock().unwrap();
        assert!(seen[1].1.contains(r#""tfaKey":"k1""#));
    }
}
