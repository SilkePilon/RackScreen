//! Spotify now playing: Authorization Code with PKCE against the user's own
//! developer app, a token file in the service user's state dir, and a poll
//! of `GET /v1/me/player` (3 s while playing, 15 s otherwise, `Retry-After`
//! on 429).

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rackscreen_core::event::{Event, MusicSource, Track};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::cover::CoverCache;
use crate::http::client;
use crate::SourceCtx;

pub const AUTHORIZE_URL: &str = "https://accounts.spotify.com/authorize";
pub const TOKEN_URL: &str = "https://accounts.spotify.com/api/token";
pub const PLAYER_URL: &str = "https://api.spotify.com/v1/me/player?additional_types=track,episode";
pub const ME_URL: &str = "https://api.spotify.com/v1/me";
/// Register exactly this in the Spotify developer dashboard.
pub const REDIRECT_URI: &str = "http://127.0.0.1:8888/callback";
pub const SCOPES: &str = "user-read-playback-state user-read-currently-playing";
/// Smallest album image side worth fetching (2 × the 84 px cover).
pub const MIN_IMAGE_PX: u64 = 168;
pub const POLL_PLAYING: Duration = Duration::from_secs(3);
pub const POLL_IDLE: Duration = Duration::from_secs(15);
/// Refresh this long before the access token runs out.
pub const REFRESH_MARGIN_SECS: i64 = 60;
/// How often a broken or missing login is looked at again.
pub const LOGIN_RETRY: Duration = Duration::from_secs(60);

// ---------------------------------------------------------------- PKCE

/// RFC 7636 `code_verifier` from random bytes: base64url, no padding
/// (32 bytes give 43 characters).
pub fn verifier_from(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

/// `S256` code challenge: base64url(SHA-256(verifier)), no padding.
pub fn challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

/// One login attempt: what to keep (verifier, state) and the URL to open.
#[derive(Clone, Debug, PartialEq)]
pub struct Login {
    pub verifier: String,
    pub state: String,
    pub url: String,
}

/// A login from given random bytes (tests) ...
pub fn login_from(client_id: &str, verifier_bytes: &[u8], state_bytes: &[u8]) -> Login {
    let verifier = verifier_from(verifier_bytes);
    let state = verifier_from(state_bytes);
    let mut u = url::Url::parse(AUTHORIZE_URL).expect("authorize url");
    u.query_pairs_mut()
        .append_pair("client_id", client_id.trim())
        .append_pair("response_type", "code")
        .append_pair("redirect_uri", REDIRECT_URI)
        .append_pair("code_challenge_method", "S256")
        .append_pair("code_challenge", &challenge(&verifier))
        .append_pair("scope", SCOPES)
        .append_pair("state", &state);
    Login {
        verifier,
        state,
        url: u.into(),
    }
}

/// ... and from the OS random source.
pub fn new_login(client_id: &str) -> Result<Login> {
    let mut v = [0u8; 32];
    let mut s = [0u8; 12];
    getrandom::fill(&mut v).map_err(|e| anyhow::anyhow!("random: {e}"))?;
    getrandom::fill(&mut s).map_err(|e| anyhow::anyhow!("random: {e}"))?;
    Ok(login_from(client_id, &v, &s))
}

#[derive(Debug, PartialEq, thiserror::Error)]
pub enum AuthError {
    #[error("that is not the address from the browser")]
    BadUrl,
    #[error("Spotify said: {0}")]
    Denied(String),
    #[error("the address belongs to another login attempt; log in again")]
    StateMismatch,
    #[error("the address has no code")]
    NoCode,
}

/// The `code` from the address the browser was sent to after login. A bare
/// query (`code=...&state=...`) is accepted too.
pub fn parse_redirect(pasted: &str, state: &str) -> Result<String, AuthError> {
    let text = pasted.trim();
    let query = match url::Url::parse(text) {
        Ok(u) => u.query().unwrap_or_default().to_string(),
        Err(_) if text.contains('=') && !text.contains(char::is_whitespace) => {
            text.trim_start_matches('?').to_string()
        }
        Err(_) => return Err(AuthError::BadUrl),
    };
    let pairs: std::collections::HashMap<String, String> =
        url::form_urlencoded::parse(query.as_bytes())
            .into_owned()
            .collect();
    if pairs.get("state").map(String::as_str) != Some(state) {
        if pairs.is_empty() {
            return Err(AuthError::BadUrl);
        }
        return Err(AuthError::StateMismatch);
    }
    if let Some(e) = pairs.get("error") {
        return Err(AuthError::Denied(e.clone()));
    }
    pairs
        .get("code")
        .filter(|c| !c.is_empty())
        .cloned()
        .ok_or(AuthError::NoCode)
}

// ---------------------------------------------------------------- tokens

/// The token file: `$XDG_STATE_HOME/rackscreen/spotify.json`, mode 0600.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TokenFile {
    pub access_token: String,
    pub refresh_token: String,
    /// Unix seconds.
    pub expires_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// Set by the service when Spotify refuses the refresh token.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl TokenFile {
    pub fn needs_refresh(&self, unix_now: i64) -> bool {
        unix_now >= self.expires_at - REFRESH_MARGIN_SECS
    }
}

pub fn code_form(client_id: &str, code: &str, verifier: &str) -> Vec<(&'static str, String)> {
    vec![
        ("grant_type", "authorization_code".into()),
        ("code", code.into()),
        ("redirect_uri", REDIRECT_URI.into()),
        ("client_id", client_id.trim().into()),
        ("code_verifier", verifier.into()),
    ]
}

pub fn refresh_form(client_id: &str, refresh_token: &str) -> Vec<(&'static str, String)> {
    vec![
        ("grant_type", "refresh_token".into()),
        ("refresh_token", refresh_token.into()),
        ("client_id", client_id.trim().into()),
    ]
}

/// Fold a token endpoint answer into the token file. Spotify may rotate the
/// refresh token; without a new one the previous one stays. An `error`
/// answer (`invalid_grant`) is an `Err` with that code.
pub fn apply_token_response(
    status: u16,
    body: &str,
    prev: Option<&TokenFile>,
    unix_now: i64,
) -> Result<TokenFile, String> {
    let v: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    if let Some(e) = v.get("error").and_then(Value::as_str) {
        return Err(e.to_string());
    }
    if !(200..300).contains(&status) {
        return Err(format!("http {status}"));
    }
    let access = v
        .get("access_token")
        .and_then(Value::as_str)
        .ok_or("no access_token")?;
    let refresh = v
        .get("refresh_token")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| prev.map(|p| p.refresh_token.clone()))
        .filter(|r| !r.is_empty())
        .ok_or("no refresh_token")?;
    let expires_in = v.get("expires_in").and_then(Value::as_i64).unwrap_or(3600);
    Ok(TokenFile {
        access_token: access.to_string(),
        refresh_token: refresh,
        expires_at: unix_now + expires_in,
        display_name: prev.and_then(|p| p.display_name.clone()),
        error: None,
    })
}

/// `/v1/me` `display_name`, falling back to the user id.
pub fn parse_display_name(body: &str) -> Option<String> {
    let v: Value = serde_json::from_str(body).ok()?;
    ["display_name", "id"]
        .iter()
        .find_map(|k| v.get(*k).and_then(Value::as_str))
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

pub fn token_path_in(state_dir: &Path) -> PathBuf {
    state_dir.join("rackscreen").join("spotify.json")
}

pub fn read_token(path: &Path) -> Option<TokenFile> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

/// Write via a temp file and rename, mode 0600; with `owner`, the created
/// directories and the file are chowned to it (setup runs under sudo).
pub fn write_token(path: &Path, tok: &TokenFile, owner: Option<(u32, u32)>) -> Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let dir = path.parent().context("token path has no parent")?;
    let mut missing = Vec::new();
    let mut d = Some(dir);
    while let Some(p) = d.filter(|p| !p.exists()) {
        missing.push(p.to_path_buf());
        d = p.parent();
    }
    std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
    let tmp = path.with_extension("json.tmp");
    let _ = std::fs::remove_file(&tmp);
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&tmp)
        .with_context(|| format!("write {}", tmp.display()))?;
    f.write_all(serde_json::to_string_pretty(tok)?.as_bytes())?;
    f.sync_all()?;
    drop(f);
    if let Some((uid, gid)) = owner {
        for p in missing.iter().chain([&tmp]) {
            std::os::unix::fs::chown(p, Some(uid), Some(gid))
                .with_context(|| format!("chown {}", p.display()))?;
        }
    }
    std::fs::rename(&tmp, path).with_context(|| format!("rename to {}", path.display()))
}

/// The setup screen's status line for the login.
pub fn login_status(tok: Option<&TokenFile>) -> String {
    match tok {
        None => "not logged in".into(),
        Some(t) if t.error.is_some() => "token expired, log in again".into(),
        Some(t) => format!(
            "logged in as {}",
            t.display_name.as_deref().unwrap_or("unknown user")
        ),
    }
}

// ---------------------------------------------------------------- player

/// One `/v1/me/player` answer, flattened.
#[derive(Clone, Debug, PartialEq)]
pub struct SpotifyItem {
    pub title: String,
    pub artist: String,
    pub device: String,
    pub duration_secs: f64,
    pub position_secs: f64,
    pub playing: bool,
    /// Album (or show) id: the cover's cache key.
    pub cover_key: Option<String>,
    pub cover_url: Option<String>,
}

/// The smallest image at least `MIN_IMAGE_PX` wide, else the largest.
pub fn cover_url(images: &Value) -> Option<String> {
    let imgs: Vec<(u64, &str)> = images
        .as_array()?
        .iter()
        .filter_map(|i| {
            let url = i.get("url")?.as_str()?;
            let w = i
                .get("width")
                .or_else(|| i.get("height"))
                .and_then(Value::as_u64)
                .unwrap_or(0);
            Some((w, url))
        })
        .collect();
    imgs.iter()
        .filter(|(w, _)| *w >= MIN_IMAGE_PX)
        .min_by_key(|(w, _)| *w)
        .or_else(|| imgs.iter().max_by_key(|(w, _)| *w))
        .map(|(_, u)| u.to_string())
}

/// `204` (or an ad, or no item) is nothing playing; a track's artist is
/// its artists joined, an episode's the show's name.
pub fn parse_player(status: u16, body: &str) -> Result<Option<SpotifyItem>> {
    if status == 204 {
        return Ok(None);
    }
    if !(200..300).contains(&status) {
        bail!("player: http {status}");
    }
    let v: Value = serde_json::from_str(body).context("player json")?;
    let Some(item) = v.get("item").filter(|i| i.is_object()) else {
        return Ok(None);
    };
    let str_of = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let episode = item.get("type").and_then(Value::as_str) == Some("episode");
    let (artist, cover_key, images) = if episode {
        let show = item.get("show").unwrap_or(&Value::Null);
        let images = item
            .get("images")
            .filter(|i| i.as_array().is_some_and(|a| !a.is_empty()))
            .or_else(|| show.get("images"))
            .unwrap_or(&Value::Null);
        (str_of(show, "name"), show.get("id"), images)
    } else {
        let artists: Vec<&str> = item
            .get("artists")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(|x| x.get("name")?.as_str()).collect())
            .unwrap_or_default();
        let album = item.get("album").unwrap_or(&Value::Null);
        (
            artists.join(", "),
            album.get("id"),
            album.get("images").unwrap_or(&Value::Null),
        )
    };
    let ms = |v: Option<&Value>| v.and_then(Value::as_f64).unwrap_or(0.0) / 1000.0;
    let cover_url = cover_url(images);
    Ok(Some(SpotifyItem {
        title: str_of(item, "name"),
        artist,
        device: v
            .get("device")
            .map(|d| str_of(d, "name"))
            .unwrap_or_default(),
        duration_secs: ms(item.get("duration_ms")),
        position_secs: ms(v.get("progress_ms")),
        playing: v
            .get("is_playing")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        cover_key: cover_url
            .as_ref()
            .and(cover_key.and_then(Value::as_str).map(str::to_string)),
        cover_url,
    }))
}

/// How long to wait before the next poll.
pub fn next_poll(playing: bool, retry_after: Option<u64>) -> Duration {
    let base = if playing { POLL_PLAYING } else { POLL_IDLE };
    match retry_after {
        Some(s) => Duration::from_secs(s.min(3600)).max(POLL_PLAYING),
        None => base,
    }
}

impl SpotifyItem {
    pub fn into_track(self, cover: Option<rackscreen_core::event::CoverRef>) -> Track {
        Track {
            source: MusicSource::Spotify,
            title: self.title,
            artist: self.artist,
            device: self.device,
            duration_secs: self.duration_secs,
            position_secs: self.position_secs,
            playing: self.playing,
            since: 0,
            cover,
        }
    }
}

// ---------------------------------------------------------------- service

/// No usable login: the screen drops the track and the source waits for
/// setup to write a new token file. Anything else is a network problem and
/// the last track stays.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct LoginError(pub String);

pub fn is_login_error(e: &anyhow::Error) -> bool {
    e.downcast_ref::<LoginError>().is_some()
}

#[derive(Clone, Debug)]
pub struct SpotifyConfig {
    pub client_id: String,
    pub token_path: PathBuf,
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// A usable access token: the file's, refreshed (and written back) when it
/// is about to run out. `Err` when there is no login or it was refused.
async fn access_token(cfg: &SpotifyConfig, force: bool) -> Result<String> {
    let Some(tok) = read_token(&cfg.token_path) else {
        return Err(LoginError(format!("not logged in ({})", cfg.token_path.display())).into());
    };
    if let Some(e) = &tok.error {
        return Err(LoginError(format!(
            "login refused earlier ({e}); log in again in setup"
        ))
        .into());
    }
    let now = unix_now();
    if !force && !tok.needs_refresh(now) {
        return Ok(tok.access_token);
    }
    let resp = client()
        .post(TOKEN_URL)
        .form(&refresh_form(&cfg.client_id, &tok.refresh_token))
        .send()
        .await
        .context("token refresh")?;
    let status = resp.status().as_u16();
    let body = resp.text().await.unwrap_or_default();
    match apply_token_response(status, &body, Some(&tok), now) {
        Ok(next) => {
            write_token(&cfg.token_path, &next, None)?;
            Ok(next.access_token)
        }
        Err(code) if code == "invalid_grant" || code == "invalid_client" => {
            let mut bad = tok.clone();
            bad.error = Some(code.clone());
            write_token(&cfg.token_path, &bad, None)?;
            Err(LoginError(format!("token refresh refused: {code}")).into())
        }
        Err(code) => bail!("token refresh failed: {code}"),
    }
}

pub async fn run_spotify(cfg: SpotifyConfig, ctx: SourceCtx) {
    let mut covers = CoverCache::default();
    let mut prev: Option<Track> = None;
    let mut last_err: Option<String> = None;
    loop {
        if ctx.shutdown.is_cancelled() {
            return;
        }
        let wait = match poll_once(&cfg, &mut covers, &mut prev).await {
            Ok((track, wait)) => {
                if last_err.take().is_some() {
                    tracing::info!("spotify: polling again");
                }
                ctx.emit(Event::Music {
                    source: MusicSource::Spotify,
                    track,
                });
                wait
            }
            Err(e) => {
                let msg = format!("{e:#}");
                if last_err.as_deref() != Some(msg.as_str()) {
                    tracing::warn!("spotify: {msg}");
                    last_err = Some(msg);
                }
                if is_login_error(&e) {
                    prev = None;
                    ctx.emit(Event::Music {
                        source: MusicSource::Spotify,
                        track: None,
                    });
                    LOGIN_RETRY
                } else {
                    // a network blip: keep the last track on screen
                    POLL_IDLE
                }
            }
        };
        tokio::select! {
            _ = ctx.shutdown.cancelled() => return,
            _ = tokio::time::sleep(wait) => {}
        }
    }
}

async fn poll_once(
    cfg: &SpotifyConfig,
    covers: &mut CoverCache,
    prev: &mut Option<Track>,
) -> Result<(Option<Track>, Duration)> {
    let mut token = access_token(cfg, false).await?;
    let mut resp = client()
        .get(PLAYER_URL)
        .bearer_auth(&token)
        .send()
        .await
        .context("player")?;
    if resp.status().as_u16() == 401 {
        token = access_token(cfg, true).await?;
        resp = client()
            .get(PLAYER_URL)
            .bearer_auth(&token)
            .send()
            .await
            .context("player")?;
    }
    let status = resp.status().as_u16();
    if status == 429 {
        let retry = resp
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.trim().parse().ok());
        tracing::debug!("spotify: rate limited, retry after {retry:?} s");
        return Ok((prev.clone(), next_poll(false, retry.or(Some(30)))));
    }
    let body = resp.text().await.unwrap_or_default();
    let Some(item) = parse_player(status, &body)? else {
        *prev = None;
        return Ok((None, next_poll(false, None)));
    };
    let cover = match (&item.cover_key, &item.cover_url) {
        (Some(key), Some(url)) => covers.fetch(key, client().get(url)).await,
        _ => None,
    };
    let playing = item.playing;
    let mut track = item.into_track(cover);
    rackscreen_core::music::stamp_since(prev.as_ref(), &mut track, unix_now());
    *prev = Some(track.clone());
    Ok((Some(track), next_poll(playing, None)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> String {
        std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures")
                .join(name),
        )
        .unwrap()
    }

    #[test]
    fn pkce_matches_the_rfc_7636_vector() {
        // RFC 7636 appendix B
        let bytes = [
            116, 24, 223, 180, 151, 153, 224, 37, 79, 250, 96, 125, 216, 173, 187, 186, 22, 212,
            37, 77, 105, 214, 191, 240, 91, 88, 5, 88, 83, 132, 141, 121,
        ];
        let v = verifier_from(&bytes);
        assert_eq!(v, "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk");
        assert_eq!(challenge(&v), "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
    }

    #[test]
    fn login_url_carries_client_challenge_scopes_and_state() {
        let l = login_from("abc123", &[1; 32], &[2; 12]);
        assert_eq!(l.verifier.len(), 43);
        let u = url::Url::parse(&l.url).unwrap();
        assert_eq!(
            format!("{}://{}{}", u.scheme(), u.host_str().unwrap(), u.path()),
            AUTHORIZE_URL
        );
        let q: std::collections::HashMap<_, _> = u.query_pairs().into_owned().collect();
        assert_eq!(q["client_id"], "abc123");
        assert_eq!(q["response_type"], "code");
        assert_eq!(q["redirect_uri"], REDIRECT_URI);
        assert_eq!(q["code_challenge_method"], "S256");
        assert_eq!(q["code_challenge"], challenge(&l.verifier));
        assert_eq!(q["scope"], SCOPES);
        assert_eq!(q["state"], l.state);
        let a = new_login("x").unwrap();
        let b = new_login("x").unwrap();
        assert_ne!(a.verifier, b.verifier, "random per login");
    }

    #[test]
    fn redirect_parsing() {
        let ok = "http://127.0.0.1:8888/callback?code=AQ_code-1&state=st8";
        assert_eq!(parse_redirect(ok, "st8"), Ok("AQ_code-1".to_string()));
        assert_eq!(
            parse_redirect(&format!("  {ok}\n"), "st8"),
            Ok("AQ_code-1".to_string()),
            "whitespace from the paste is ignored"
        );
        assert_eq!(
            parse_redirect("code=c2&state=st8", "st8"),
            Ok("c2".to_string()),
            "a bare query"
        );
        assert_eq!(parse_redirect(ok, "other"), Err(AuthError::StateMismatch));
        assert_eq!(
            parse_redirect(
                "http://127.0.0.1:8888/callback?error=access_denied&state=st8",
                "st8"
            ),
            Err(AuthError::Denied("access_denied".into()))
        );
        assert_eq!(
            parse_redirect("http://127.0.0.1:8888/callback?state=st8", "st8"),
            Err(AuthError::NoCode)
        );
        assert_eq!(parse_redirect("", "st8"), Err(AuthError::BadUrl));
        assert_eq!(parse_redirect("hello there", "st8"), Err(AuthError::BadUrl));
    }

    #[test]
    fn token_exchange_refresh_and_rotation() {
        let first = apply_token_response(200, &fixture("spotify-token.json"), None, 1000).unwrap();
        assert_eq!(
            (
                first.access_token.as_str(),
                first.refresh_token.as_str(),
                first.expires_at
            ),
            ("BQ-new", "AQ-first", 4600)
        );
        assert!(!first.needs_refresh(4000) && first.needs_refresh(4541));
        let mut named = first.clone();
        named.display_name = Some("Silke".into());
        let refreshed = apply_token_response(
            200,
            &fixture("spotify-token-refresh.json"),
            Some(&named),
            5000,
        )
        .unwrap();
        assert_eq!(refreshed.access_token, "BQ-refreshed");
        assert_eq!(refreshed.refresh_token, "AQ-first", "no new one: kept");
        assert_eq!(refreshed.display_name.as_deref(), Some("Silke"), "kept");
        assert_eq!(refreshed.expires_at, 8600);
        let rotated = apply_token_response(
            200,
            &fixture("spotify-token-rotated.json"),
            Some(&named),
            5000,
        )
        .unwrap();
        assert_eq!(rotated.refresh_token, "AQ-rotated", "rotated: replaced");
        assert_eq!(
            apply_token_response(
                400,
                &fixture("spotify-token-revoked.json"),
                Some(&named),
                5000
            ),
            Err("invalid_grant".to_string())
        );
        assert_eq!(
            apply_token_response(503, "<html>", Some(&named), 5000),
            Err("http 503".to_string())
        );
        assert_eq!(
            parse_display_name(&fixture("spotify-me.json")).as_deref(),
            Some("Silke")
        );
        assert_eq!(
            parse_display_name(r#"{"display_name":null,"id":"u1"}"#).as_deref(),
            Some("u1")
        );
    }

    #[test]
    fn forms_carry_what_the_token_endpoint_wants() {
        let f = code_form("cid", "the-code", "ver");
        assert!(f.contains(&("grant_type", "authorization_code".into())));
        assert!(f.contains(&("code", "the-code".into())));
        assert!(f.contains(&("redirect_uri", REDIRECT_URI.into())));
        assert!(f.contains(&("client_id", "cid".into())));
        assert!(f.contains(&("code_verifier", "ver".into())));
        let r = refresh_form("cid", "rt");
        assert!(r.contains(&("grant_type", "refresh_token".into())));
        assert!(r.contains(&("refresh_token", "rt".into())));
        assert!(r.contains(&("client_id", "cid".into())));
    }

    #[test]
    fn token_file_round_trips_with_mode_0600() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = token_path_in(dir.path());
        assert_eq!(path, dir.path().join("rackscreen/spotify.json"));
        assert_eq!(read_token(&path), None);
        let tok = TokenFile {
            access_token: "a".into(),
            refresh_token: "r".into(),
            expires_at: 99,
            display_name: Some("Silke".into()),
            error: None,
        };
        write_token(&path, &tok, None).unwrap();
        assert_eq!(read_token(&path), Some(tok.clone()));
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        std::fs::write(&path, "not json").unwrap();
        assert_eq!(read_token(&path), None);
        assert_eq!(login_status(None), "not logged in");
        assert_eq!(login_status(Some(&tok)), "logged in as Silke");
        let bad = TokenFile {
            error: Some("invalid_grant".into()),
            ..tok
        };
        assert_eq!(login_status(Some(&bad)), "token expired, log in again");
    }

    #[test]
    fn player_track_paused_204_episode_ad_and_missing_images() {
        let p = parse_player(200, &fixture("spotify-player-playing.json"))
            .unwrap()
            .unwrap();
        assert_eq!(
            p,
            SpotifyItem {
                title: "Holocene".into(),
                artist: "Bon Iver, Guest".into(),
                device: "Kitchen speaker".into(),
                duration_secs: 337.0,
                position_secs: 134.25,
                playing: true,
                cover_key: Some("1dRVv2ZNkbRbOmbpDIqv7r".into()),
                cover_url: Some("https://i.scdn.co/image/300".into()),
            }
        );
        let paused = parse_player(200, &fixture("spotify-player-paused.json"))
            .unwrap()
            .unwrap();
        assert!(!paused.playing && paused.position_secs == 200.0);
        assert_eq!(parse_player(204, "").unwrap(), None);
        let ep = parse_player(200, &fixture("spotify-player-episode.json"))
            .unwrap()
            .unwrap();
        assert_eq!(
            (ep.title.as_str(), ep.artist.as_str(), ep.duration_secs),
            ("The Mountain", "Radiolab", 3000.0)
        );
        assert_eq!(ep.cover_key.as_deref(), Some("show1"));
        assert_eq!(
            ep.cover_url.as_deref(),
            Some("https://i.scdn.co/image/ep300")
        );
        assert_eq!(
            parse_player(200, &fixture("spotify-player-ad.json")).unwrap(),
            None
        );
        let bare = parse_player(200, &fixture("spotify-player-no-images.json"))
            .unwrap()
            .unwrap();
        assert_eq!(bare.cover_url, None);
        assert!(parse_player(500, "oops").is_err());
        assert!(parse_player(200, "{").is_err());
        let t = p.into_track(None);
        assert_eq!((t.source, t.since), (MusicSource::Spotify, 0));
    }

    #[test]
    fn cover_url_prefers_the_smallest_big_enough() {
        let v: Value = serde_json::from_str(
            r#"[{"url":"a","width":640},{"url":"b","width":170},{"url":"c","width":64}]"#,
        )
        .unwrap();
        assert_eq!(cover_url(&v).as_deref(), Some("b"));
        let small: Value =
            serde_json::from_str(r#"[{"url":"c","width":64},{"url":"d","width":120}]"#).unwrap();
        assert_eq!(cover_url(&small).as_deref(), Some("d"), "else the largest");
        assert_eq!(cover_url(&Value::Null), None);
    }

    #[test]
    fn only_login_problems_count_as_login_errors() {
        assert!(is_login_error(&LoginError("not logged in".into()).into()));
        let net = anyhow::anyhow!("dns").context("player");
        assert!(!is_login_error(&net));
    }

    #[test]
    fn polls_fast_while_playing_and_back_off_on_429() {
        assert_eq!(next_poll(true, None), POLL_PLAYING);
        assert_eq!(next_poll(false, None), POLL_IDLE);
        assert_eq!(next_poll(true, Some(42)), Duration::from_secs(42));
        assert_eq!(next_poll(true, Some(0)), POLL_PLAYING, "never faster");
        assert_eq!(
            next_poll(false, Some(100_000)),
            Duration::from_secs(3600),
            "capped at an hour"
        );
    }
}
