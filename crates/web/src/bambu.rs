//! `/api/bambu`: the Bambu Lab account login for the printer's cloud mode.
//! The password goes to Bambu Lab and nowhere else; what is kept is the
//! token file the printer source reads.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::extract::State;
use axum::Json;
use rackscreen_app::config::{BambuMode, Config};
use rackscreen_sources::bambu_cloud::{
    self as cloud, CloudError, Endpoints, LoginStep, Region, TokenFile,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{ApiError, ApiResult, AppState};

/// How long a login waiting for its code is kept.
pub const PENDING_TTL: Duration = Duration::from_secs(600);

/// A login that needs a code next.
pub struct Pending {
    account: String,
    region: Region,
    /// `None`: an emailed code; `Some`: an authenticator code.
    tfa_key: Option<String>,
    created: Instant,
}

pub struct BambuState {
    pending: Mutex<Option<Pending>>,
    token_path: Option<PathBuf>,
    /// Where the login goes; `None` is Bambu Lab's own hosts by region.
    endpoints: Option<Endpoints>,
}

impl BambuState {
    pub fn real() -> Self {
        Self::new(rackscreen_app::bambu::token_path(), None)
    }

    pub fn new(token_path: Option<PathBuf>, endpoints: Option<Endpoints>) -> Self {
        Self {
            pending: Mutex::new(None),
            token_path,
            endpoints,
        }
    }

    fn endpoints(&self, region: Region) -> Endpoints {
        self.endpoints
            .clone()
            .unwrap_or_else(|| Endpoints::for_region(region))
    }

    fn token_path(&self) -> Result<&PathBuf, ApiError> {
        self.token_path
            .as_ref()
            .ok_or_else(|| ApiError::internal("no home directory for the token file"))
    }

    fn pending_kind(&self) -> Option<&'static str> {
        let p = self.pending.lock().unwrap();
        p.as_ref()
            .filter(|p| p.created.elapsed() < PENDING_TTL)
            .map(|p| if p.tfa_key.is_some() { "tfa" } else { "code" })
    }
}

fn now() -> i64 {
    rackscreen_app::hub::unix_now() as i64
}

fn load(state: &AppState) -> Result<Config, ApiError> {
    Config::load_or_default(&state.hub.config_path)
        .map_err(|e| ApiError::internal(format!("{e:#}")))
}

/// A failed request in the words the page shows.
fn api_error(e: anyhow::Error) -> ApiError {
    match e.downcast_ref::<CloudError>() {
        Some(CloudError::Unexpected(_)) | None => {
            tracing::warn!("bambu: {e:#}");
            ApiError::unavailable(format!("{e:#}"))
        }
        Some(c) => ApiError::bad_request(c.to_string()),
    }
}

fn status_json(state: &AppState) -> Result<Value, ApiError> {
    let cfg = load(state)?;
    let tok = state
        .bambu
        .token_path()
        .ok()
        .and_then(|p| cloud::read_token(p));
    let now = now();
    let status = cloud::login_status(tok.as_ref(), now);
    let mut c = status.chars();
    let status = c
        .next()
        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
        .unwrap_or_default();
    Ok(json!({
        "mode": if cfg.bambu.mode == BambuMode::Cloud { "cloud" } else { "lan" },
        "logged_in": tok.as_ref().is_some_and(|t| t.usable(now)),
        "account": tok.as_ref().map(|t| t.account.clone()),
        "region": tok.as_ref().map(|t| t.region),
        "expires_at": tok.as_ref().map(|t| t.expires_at),
        "status": status,
        "pending": state.bambu.pending_kind(),
    }))
}

pub async fn get_bambu(State(state): State<Arc<AppState>>) -> ApiResult<Value> {
    Ok(Json(status_json(&state)?))
}

#[derive(Deserialize)]
pub struct LoginBody {
    pub account: String,
    pub password: String,
    #[serde(default)]
    pub region: Option<Region>,
}

async fn save(
    state: &AppState,
    region: Region,
    account: &str,
    token: String,
) -> Result<Value, ApiError> {
    let ep = state.bambu.endpoints(region);
    let tok: TokenFile = cloud::finish(&cloud::client(), &ep, region, account, token, now())
        .await
        .map_err(api_error)?;
    cloud::write_token(state.bambu.token_path()?, &tok, None)
        .map_err(|e| ApiError::internal(format!("{e:#}")))?;
    *state.bambu.pending.lock().unwrap() = None;
    tracing::info!("bambu: logged in as {}", tok.account);
    status_json(state)
}

pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(body): Json<LoginBody>,
) -> ApiResult<Value> {
    let account = body.account.trim().to_string();
    if account.is_empty() || body.password.is_empty() {
        return Err(ApiError::bad_request("enter the email and the password"));
    }
    let region = match body.region {
        Some(r) => r,
        None => load(&state)?.bambu.region,
    };
    state.bambu.token_path()?;
    let ep = state.bambu.endpoints(region);
    let step = cloud::login(&cloud::client(), &ep, &account, &body.password)
        .await
        .map_err(api_error)?;
    let tfa_key = match step {
        LoginStep::Done(token) => return Ok(Json(save(&state, region, &account, token).await?)),
        LoginStep::EmailCode => None,
        LoginStep::Tfa { tfa_key } => Some(tfa_key),
    };
    let needs = if tfa_key.is_some() { "tfa" } else { "code" };
    *state.bambu.pending.lock().unwrap() = Some(Pending {
        account,
        region,
        tfa_key,
        created: Instant::now(),
    });
    Ok(Json(json!({ "needs": needs })))
}

#[derive(Deserialize)]
pub struct CodeBody {
    pub code: String,
}

/// The pending login's account, region and 2FA key.
fn pending(state: &AppState) -> Result<(String, Region, Option<String>), ApiError> {
    let p = state.bambu.pending.lock().unwrap();
    p.as_ref()
        .filter(|p| p.created.elapsed() < PENDING_TTL)
        .map(|p| (p.account.clone(), p.region, p.tfa_key.clone()))
        .ok_or_else(|| {
            ApiError::bad_request("log in with the password first (it expires after 10 min)")
        })
}

pub async fn code(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CodeBody>,
) -> ApiResult<Value> {
    let code: String = body.code.chars().filter(|c| !c.is_whitespace()).collect();
    if code.is_empty() || code.len() > 16 || !code.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err(ApiError::bad_request(
            "enter the code from the email or the app",
        ));
    }
    let (account, region, tfa_key) = pending(&state)?;
    let ep = state.bambu.endpoints(region);
    let http = cloud::client();
    let token = match &tfa_key {
        None => cloud::submit_code(&http, &ep, &account, &code).await,
        Some(key) => cloud::submit_tfa(&http, &ep, key, &code).await,
    }
    .map_err(api_error)?;
    Ok(Json(save(&state, region, &account, token).await?))
}

/// A new emailed code for the pending login.
pub async fn resend(State(state): State<Arc<AppState>>) -> ApiResult<Value> {
    let (account, region, tfa_key) = pending(&state)?;
    if tfa_key.is_some() {
        return Err(ApiError::bad_request(
            "this account uses an authenticator app; take the code from there",
        ));
    }
    cloud::send_code(&cloud::client(), &state.bambu.endpoints(region), &account)
        .await
        .map_err(api_error)?;
    Ok(Json(json!({ "ok": true })))
}

/// The printers on the account, for the picker.
pub async fn printers(State(state): State<Arc<AppState>>) -> ApiResult<Value> {
    let tok = cloud::read_token(state.bambu.token_path()?)
        .filter(|t| t.usable(now()))
        .ok_or_else(|| ApiError::bad_request("log in to Bambu Lab first"))?;
    let list = cloud::printers(
        &cloud::client(),
        &state.bambu.endpoints(tok.region),
        &tok.token,
    )
    .await
    .map_err(api_error)?;
    Ok(Json(json!({ "printers": list })))
}

pub async fn logout(State(state): State<Arc<AppState>>) -> ApiResult<Value> {
    let path = state.bambu.token_path()?;
    match std::fs::remove_file(path) {
        Ok(()) => tracing::info!("bambu: logged out"),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => {
            return Err(ApiError::internal(format!(
                "remove {}: {e}",
                path.display()
            )))
        }
    }
    *state.bambu.pending.lock().unwrap() = None;
    Ok(Json(status_json(&state)?))
}
