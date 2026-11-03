//! `/api/update`: the latest release (checked at most hourly) and a
//! self-update job that runs `sudo -n /usr/local/bin/rackscreen update`.

use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::State;
use axum::Json;
use rackscreen_ops::update::{fetch_latest, is_newer};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::{ApiError, ApiResult, AppState};

pub const CHECK_EVERY: Duration = Duration::from_secs(3600);
pub const SUDO_CHECK_EVERY: Duration = Duration::from_secs(60);
pub const BINARY: &str = "/usr/local/bin/rackscreen";

#[derive(Default)]
pub struct UpdateState {
    /// When the release was last looked up, and the tag found (None: unknown).
    pub latest: Option<(Instant, Option<String>)>,
    pub sudo_ok: Option<(Instant, bool)>,
    pub job_state: JobState,
    pub job_log: Vec<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum JobState {
    #[default]
    Idle,
    Running,
    Done,
    Failed,
}

impl JobState {
    pub fn name(self) -> &'static str {
        match self {
            JobState::Idle => "idle",
            JobState::Running => "running",
            JobState::Done => "done",
            JobState::Failed => "failed",
        }
    }
}

/// `sudo -n -l <binary> update` succeeds when the sudoers drop-in allows it.
async fn can_self_update() -> bool {
    tokio::process::Command::new("sudo")
        .args(["-n", "-l", BINARY, "update"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await
        .is_ok_and(|s| s.success())
}

pub async fn get_update(State(state): State<Arc<AppState>>) -> ApiResult<Value> {
    let current = state.hub.version;
    let need_check = state
        .update
        .lock()
        .unwrap()
        .latest
        .as_ref()
        .is_none_or(|(t, _)| t.elapsed() >= CHECK_EVERY);
    if need_check {
        let found = tokio::task::spawn_blocking(|| fetch_latest().ok().map(|r| r.tag))
            .await
            .ok()
            .flatten();
        state.update.lock().unwrap().latest = Some((Instant::now(), found));
    }
    let need_sudo = state
        .update
        .lock()
        .unwrap()
        .sudo_ok
        .is_none_or(|(t, _)| t.elapsed() >= SUDO_CHECK_EVERY);
    if need_sudo {
        let ok = can_self_update().await;
        state.update.lock().unwrap().sudo_ok = Some((Instant::now(), ok));
    }
    let u = state.update.lock().unwrap();
    let latest = u.latest.as_ref().and_then(|(_, l)| l.clone());
    Ok(Json(json!({
        "current": current,
        "latest": latest,
        "newer": latest.as_deref().is_some_and(|l| is_newer(current, l)),
        "can_self_update": u.sudo_ok.is_some_and(|(_, ok)| ok),
        "job": { "state": u.job_state.name(), "log": u.job_log },
    })))
}

pub async fn post_update(State(state): State<Arc<AppState>>) -> ApiResult<Value> {
    {
        let mut u = state.update.lock().unwrap();
        if u.job_state == JobState::Running {
            return Err(ApiError::bad_request("an update is already running"));
        }
        u.job_state = JobState::Running;
        u.job_log.clear();
        u.job_log.push(format!("$ sudo -n {BINARY} update"));
    }
    let st = state.clone();
    tokio::spawn(async move {
        let result = run_job(&st).await;
        let mut u = st.update.lock().unwrap();
        match result {
            Ok(true) => u.job_state = JobState::Done,
            Ok(false) => u.job_state = JobState::Failed,
            Err(e) => {
                u.job_log.push(format!("error: {e:#}"));
                u.job_state = JobState::Failed;
            }
        }
    });
    Ok(Json(json!({ "ok": true })))
}

async fn run_job(state: &Arc<AppState>) -> anyhow::Result<bool> {
    let mut child = tokio::process::Command::new("sudo")
        .args(["-n", BINARY, "update"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;
    let stdout = child.stdout.take().expect("piped");
    let stderr = child.stderr.take().expect("piped");
    let out_state = state.clone();
    let out = tokio::spawn(async move {
        let mut lines = BufReader::new(stdout).lines();
        while let Ok(Some(l)) = lines.next_line().await {
            tracing::info!("update: {l}");
            out_state.update.lock().unwrap().job_log.push(l);
        }
    });
    let err_state = state.clone();
    let err = tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(l)) = lines.next_line().await {
            tracing::warn!("update: {l}");
            err_state.update.lock().unwrap().job_log.push(l);
        }
    });
    let status = child.wait().await?;
    let _ = out.await;
    let _ = err.await;
    Ok(status.success())
}
