//! `/api/update`: the latest release (checked at most hourly, or on
//! `?force=1` from Check again, at most every `FORCE_MIN`) and a
//! self-update job that runs `sudo -n /usr/local/bin/rackscreen update`.
//!
//! The update restarts this very service, so the job's state and log are
//! kept in `<state dir>/rackscreen/update.json`: after the restart the new
//! process reads it back and reports `done` (it now runs a newer version
//! than the job started from) or `failed`.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::{Query, State};
use axum::Json;
use rackscreen_ops::update::{fetch_latest, is_newer};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::{ApiError, ApiResult, AppState};

pub const CHECK_EVERY: Duration = Duration::from_secs(3600);
/// Check again looks up the release at most this often: GitHub allows 60
/// unauthenticated requests an hour.
pub const FORCE_MIN: Duration = Duration::from_secs(15);
pub const SUDO_CHECK_EVERY: Duration = Duration::from_secs(60);
pub const BINARY: &str = "/usr/local/bin/rackscreen";

#[derive(Default)]
pub struct UpdateState {
    /// When the release was last looked up, and the tag found (None: unknown).
    pub latest: Option<(Instant, Option<String>)>,
    pub sudo_ok: Option<(Instant, bool)>,
    pub job: Job,
    /// Where the job is kept across the restart (None: no home dir).
    pub file: Option<PathBuf>,
}

/// The self-update job as persisted.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Job {
    pub state: JobState,
    /// The version that started the job.
    #[serde(default)]
    pub from: String,
    /// The release it was meant to install, when known.
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub log: Vec<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
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

/// The most log lines kept (the update prints a few dozen).
const MAX_LOG: usize = 500;

/// `<state dir>/rackscreen/update.json` of the service user.
pub fn job_path() -> Option<PathBuf> {
    rackscreen_app::play::state_dir().map(|d| d.join("rackscreen").join("update.json"))
}

impl Job {
    /// The job in `path` as seen by a process running `current`. A job that
    /// was still running when the old process went away is over now: done if
    /// this process runs a newer version than the one that started it.
    pub fn load(path: &Path, current: &str) -> Job {
        let Ok(text) = std::fs::read_to_string(path) else {
            return Job::default();
        };
        let Ok(mut job) = serde_json::from_str::<Job>(&text) else {
            return Job::default();
        };
        if job.state == JobState::Running {
            if is_newer(&job.from, current) {
                job.state = JobState::Done;
                job.log.push(format!("now running {current}"));
            } else {
                job.state = JobState::Failed;
                job.log.push(format!(
                    "the service restarted and still runs {current}; the update did not finish"
                ));
            }
            // settle it on disk so a later restart does not decide again
            let _ = job.save(path);
        } else if job.from != current
            && job.target.as_deref().map(|t| t.trim_start_matches('v')) != Some(current)
        {
            // a job an older process finished, for a release this one is not
            // running: its log says nothing about now
            return Job::default();
        }
        job
    }

    /// Write atomically (temp file + rename).
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(
            &tmp,
            serde_json::to_vec(self).map_err(std::io::Error::other)?,
        )?;
        std::fs::rename(&tmp, path)
    }

    fn push(&mut self, line: String) {
        self.log.push(line);
        if self.log.len() > MAX_LOG {
            let extra = self.log.len() - MAX_LOG;
            self.log.drain(..extra);
        }
    }
}

impl UpdateState {
    /// The state for a process running `current`, with the job read back.
    pub fn load(file: Option<PathBuf>, current: &str) -> UpdateState {
        let job = file
            .as_deref()
            .map(|f| Job::load(f, current))
            .unwrap_or_default();
        UpdateState {
            job,
            file,
            ..UpdateState::default()
        }
    }

    fn persist(&self) {
        if let Some(f) = &self.file {
            if let Err(e) = self.job.save(f) {
                tracing::warn!("update: cannot write {}: {e}", f.display());
            }
        }
    }

    /// Add a log line and keep it on disk (the next line may never come).
    pub fn log(&mut self, line: String) {
        self.job.push(line);
        self.persist();
    }

    pub fn finish(&mut self, state: JobState) {
        self.job.state = state;
        self.persist();
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

#[derive(Deserialize, Default)]
pub struct UpdateQuery {
    /// Look the release up now instead of trusting the hourly cache.
    #[serde(default)]
    force: Option<String>,
}

pub async fn get_update(
    State(state): State<Arc<AppState>>,
    Query(q): Query<UpdateQuery>,
) -> ApiResult<Value> {
    let current = state.hub.version;
    let every = if q.force.is_some() {
        FORCE_MIN
    } else {
        CHECK_EVERY
    };
    let need_check = state
        .update
        .lock()
        .unwrap()
        .latest
        .as_ref()
        .is_none_or(|(t, _)| t.elapsed() >= every);
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
        "job": { "state": u.job.state.name(), "log": u.job.log },
    })))
}

pub async fn post_update(State(state): State<Arc<AppState>>) -> ApiResult<Value> {
    {
        let mut u = state.update.lock().unwrap();
        if u.job.state == JobState::Running {
            return Err(ApiError::bad_request("an update is already running"));
        }
        let target = u.latest.as_ref().and_then(|(_, l)| l.clone());
        u.job = Job {
            state: JobState::Running,
            from: state.hub.version.to_string(),
            target,
            log: Vec::new(),
        };
        u.log(format!("$ sudo -n {BINARY} update"));
    }
    let st = state.clone();
    tokio::spawn(async move {
        let result = run_job(&st).await;
        let mut u = st.update.lock().unwrap();
        match result {
            Ok(true) => u.finish(JobState::Done),
            Ok(false) => u.finish(JobState::Failed),
            Err(e) => {
                u.log(format!("error: {e:#}"));
                u.finish(JobState::Failed);
            }
        }
    });
    Ok(Json(json!({ "ok": true })))
}

async fn run_job(state: &Arc<AppState>) -> anyhow::Result<bool> {
    // Its own process group, so a signal aimed at this process (the
    // restart the update itself asks for) does not take the job down with
    // it before it is done. The persisted job covers the rest.
    let mut child = tokio::process::Command::new("sudo")
        .args(["-n", BINARY, "update"])
        .process_group(0)
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
            out_state.update.lock().unwrap().log(l);
        }
    });
    let err_state = state.clone();
    let err = tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(l)) = lines.next_line().await {
            tracing::warn!("update: {l}");
            err_state.update.lock().unwrap().log(l);
        }
    });
    let status = child.wait().await?;
    let _ = out.await;
    let _ = err.await;
    Ok(status.success())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_running_job_is_settled_by_the_version_after_the_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rackscreen").join("update.json");
        let mut u = UpdateState::load(Some(path.clone()), "0.12.0");
        assert_eq!(u.job, Job::default(), "no file yet");
        u.job = Job {
            state: JobState::Running,
            from: "0.12.0".into(),
            target: Some("v0.13.0".into()),
            log: vec![],
        };
        u.log("downloading".into());
        // the service restarts into the new version
        let after = UpdateState::load(Some(path.clone()), "0.13.0");
        assert_eq!(after.job.state, JobState::Done);
        assert_eq!(after.job.log[0], "downloading");
        assert!(after.job.log.last().unwrap().contains("0.13.0"));
        // settled on disk: loading again does not change it
        assert_eq!(Job::load(&path, "0.13.0"), after.job);

        let mut u = UpdateState::load(Some(path.clone()), "0.13.0");
        u.job = Job {
            state: JobState::Running,
            from: "0.13.0".into(),
            target: Some("v0.14.0".into()),
            log: vec![],
        };
        u.log("x".into());
        let after = UpdateState::load(Some(path.clone()), "0.13.0");
        assert_eq!(after.job.state, JobState::Failed);

        // a finished job is reported as it was
        let mut u = UpdateState::load(Some(path.clone()), "0.13.0");
        u.finish(JobState::Done);
        assert_eq!(Job::load(&path, "0.13.0").state, JobState::Done);
        // a job an older process finished for another release is forgotten
        u.job = Job {
            state: JobState::Done,
            from: "0.14.1".into(),
            target: Some("v0.14.4".into()),
            log: vec!["old".into()],
        };
        u.persist();
        assert_eq!(Job::load(&path, "0.16.1"), Job::default());
        assert_eq!(
            Job::load(&path, "0.14.4").state,
            JobState::Done,
            "the one it installed"
        );
        std::fs::write(&path, "not json").unwrap();
        assert_eq!(Job::load(&path, "0.13.0"), Job::default());
    }

    #[test]
    fn the_log_is_capped() {
        let mut j = Job::default();
        for i in 0..MAX_LOG + 10 {
            j.push(i.to_string());
        }
        assert_eq!(j.log.len(), MAX_LOG);
        assert_eq!(j.log[0], "10");
    }
}
