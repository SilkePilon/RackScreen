//! GitHub poller: the contribution calendar (GraphQL), the public events feed
//! for splashes, and Actions runs for the repos pushed to lately.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use chrono::{Local, NaiveDate, TimeZone};
use rackscreen_core::event::{CiRun, CiStatus, Event, LinkTarget};
use serde_json::{json, Value};

use crate::electricity::{backoff_secs, token_rejected, HttpStatus};
use crate::http::client;
use crate::SourceCtx;

const GRAPHQL: &str = "https://api.github.com/graphql";
const API: &str = "https://api.github.com";
const CALENDAR_QUERY: &str = "query($from: DateTime!, $to: DateTime!) { viewer { login contributionsCollection(from: $from, to: $to) { contributionCalendar { weeks { contributionDays { date contributionCount } } } } } }";
/// Days in the activity window (the 30-day best sets the outer ring).
const WINDOW_DAYS: i64 = 30;
/// The calendar is refreshed this often; events every `poll_secs`.
const CALENDAR_SECS: u64 = 300;
/// Actions runs are checked for at most this many recently pushed repos.
const MAX_RUN_REPOS: usize = 5;

/// Runs kept for the ci role, and repos discovered when `ci_repos` is empty.
pub const CI_PER_PAGE: usize = 12;
pub const CI_MAX_REPOS: usize = 10;
const CI_RECENT_PUSH_SECS: i64 = 7 * 86_400;
/// A repo that answers 404/403 is warned about this often.
const CI_WARN_SECS: u64 = 3600;

#[derive(Clone, Debug)]
pub struct GithubConfig {
    pub token: String,
    pub poll_secs: u64,
    /// Repos the ci role watches; empty discovers the ones pushed this week.
    pub ci_repos: Vec<String>,
}

/// Thirty dates ending on `today`, oldest first.
pub fn window_days(today: NaiveDate) -> Vec<NaiveDate> {
    (0..WINDOW_DAYS)
        .rev()
        .map(|back| today - chrono::Duration::days(back))
        .collect()
}

/// Midnight at the start of `date` in `tz`. A DST gap can swallow local
/// midnight (Brazil used to spring forward at 00:00), so the first instant of
/// the day after the jump stands in for it.
fn local_midnight<Tz: TimeZone>(date: NaiveDate, tz: &Tz) -> chrono::DateTime<Tz> {
    let midnight = date.and_hms_opt(0, 0, 0).expect("00:00:00 is a valid time");
    for minutes in [0, 60, 120] {
        let at = midnight + chrono::Duration::minutes(minutes);
        if let Some(dt) = tz
            .from_local_datetime(&at)
            .single()
            .or_else(|| tz.from_local_datetime(&at).earliest())
        {
            return dt;
        }
    }
    tz.from_utc_datetime(&midnight)
}

/// GraphQL `from`/`to` for the window: local midnight of the first day and of
/// the day after the last, with the zone's real offset. `window` must not be
/// empty.
pub fn calendar_bounds<Tz: chrono::TimeZone>(window: &[NaiveDate], tz: &Tz) -> (String, String)
where
    Tz::Offset: std::fmt::Display,
{
    let stamp = |date: NaiveDate| {
        local_midnight(date, tz).to_rfc3339_opts(chrono::SecondsFormat::Secs, false)
    };
    let from = stamp(window[0]);
    let to = stamp(window[window.len() - 1] + chrono::Duration::days(1));
    (from, to)
}

/// `(login, days)` from the GraphQL response; every date in `window` gets a
/// count, zero when the calendar has no entry.
pub fn parse_calendar(json: &str, window: &[NaiveDate]) -> Result<(String, Vec<(String, u32)>)> {
    let v: Value = serde_json::from_str(json).context("calendar json")?;
    if let Some(errs) = v.get("errors").and_then(Value::as_array) {
        let msg = errs
            .first()
            .and_then(|e| e.get("message"))
            .and_then(Value::as_str)
            .unwrap_or("unknown error");
        return Err(anyhow!("graphql: {msg}"));
    }
    let viewer = v
        .get("data")
        .and_then(|d| d.get("viewer"))
        .context("data.viewer missing")?;
    let login = viewer
        .get("login")
        .and_then(Value::as_str)
        .context("viewer.login missing")?
        .to_string();
    let mut counts: HashMap<String, u32> = HashMap::new();
    let weeks = viewer
        .pointer("/contributionsCollection/contributionCalendar/weeks")
        .and_then(Value::as_array)
        .context("weeks missing")?;
    for w in weeks {
        for day in w
            .get("contributionDays")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if let (Some(date), Some(n)) = (
                day.get("date").and_then(Value::as_str),
                day.get("contributionCount").and_then(Value::as_u64),
            ) {
                counts.insert(date.to_string(), n as u32);
            }
        }
    }
    let days = window
        .iter()
        .map(|d| {
            let key = d.format("%Y-%m-%d").to_string();
            let n = counts.get(&key).copied().unwrap_or(0);
            (key, n)
        })
        .collect();
    Ok((login, days))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GhKind {
    Push(u32),
    Star,
    Merge,
    Release(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GhEvent {
    pub id: String,
    pub repo: String,
    pub kind: GhKind,
}

/// The events we splash on, newest first as GitHub returns them; everything
/// else in the feed is dropped.
pub fn parse_events(json: &str) -> Result<Vec<GhEvent>> {
    let v: Value = serde_json::from_str(json).context("events json")?;
    let arr = v.as_array().context("events is not an array")?;
    let mut out = Vec::new();
    for e in arr {
        let id = e
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let repo = e
            .pointer("/repo/name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let action = e.pointer("/payload/action").and_then(Value::as_str);
        let kind = match e.get("type").and_then(Value::as_str) {
            Some("PushEvent") => GhKind::Push(
                e.pointer("/payload/commits")
                    .and_then(Value::as_array)
                    .map_or(1, |c| c.len().max(1) as u32),
            ),
            Some("WatchEvent") if action == Some("started") => GhKind::Star,
            Some("PullRequestEvent")
                if action == Some("closed")
                    && e.pointer("/payload/pull_request/merged")
                        .and_then(Value::as_bool)
                        .unwrap_or(false) =>
            {
                GhKind::Merge
            }
            Some("ReleaseEvent") if action == Some("published") => GhKind::Release(
                e.pointer("/payload/release/tag_name")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
            ),
            _ => continue,
        };
        out.push(GhEvent { id, repo, kind });
    }
    Ok(out)
}

/// Repos with a push in the page, most recent first, at most `MAX_RUN_REPOS`.
pub fn pushed_repos(events: &[GhEvent]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for e in events {
        if matches!(e.kind, GhKind::Push(_)) && !out.contains(&e.repo) {
            out.push(e.repo.clone());
            if out.len() == MAX_RUN_REPOS {
                break;
            }
        }
    }
    out
}

/// Emits one model event per feed event not seen before; the first page only
/// primes so a restart never replays yesterday's pushes.
#[derive(Debug, Default)]
pub struct EventTracker {
    primed: bool,
    seen: HashSet<String>,
}

impl EventTracker {
    pub fn diff(&mut self, events: &[GhEvent]) -> Vec<Event> {
        let mut out = Vec::new();
        if self.primed {
            // the feed is newest first; emit oldest first so splashes queue in order
            for e in events.iter().rev() {
                if self.seen.contains(&e.id) {
                    continue;
                }
                out.push(match &e.kind {
                    GhKind::Push(n) => Event::GithubPush {
                        repo: e.repo.clone(),
                        commits: *n,
                    },
                    GhKind::Star => Event::GithubStar {
                        repo: e.repo.clone(),
                    },
                    GhKind::Merge => Event::GithubMerge {
                        repo: e.repo.clone(),
                    },
                    GhKind::Release(tag) => Event::GithubRelease {
                        repo: e.repo.clone(),
                        tag: tag.clone(),
                    },
                });
            }
        }
        self.seen.extend(events.iter().map(|e| e.id.clone()));
        self.primed = true;
        out
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Run {
    pub id: u64,
    pub status: String,
    pub conclusion: Option<String>,
}

pub fn parse_runs(json: &str) -> Result<Vec<Run>> {
    let v: Value = serde_json::from_str(json).context("runs json")?;
    let arr = v
        .get("workflow_runs")
        .and_then(Value::as_array)
        .context("workflow_runs missing")?;
    Ok(arr
        .iter()
        .filter_map(|r| {
            Some(Run {
                id: r.get("id")?.as_u64()?,
                status: r.get("status")?.as_str()?.to_string(),
                conclusion: r
                    .get("conclusion")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            })
        })
        .collect())
}

/// Reports each run once, the first time it is seen completed; a repo's first
/// page only primes.
#[derive(Debug, Default)]
pub struct RunTracker {
    primed: HashSet<String>,
    reported: HashSet<u64>,
}

impl RunTracker {
    pub fn diff(&mut self, repo: &str, runs: &[Run]) -> Vec<Event> {
        let completed = runs.iter().filter(|r| r.status == "completed");
        if !self.primed.contains(repo) {
            self.primed.insert(repo.to_string());
            self.reported.extend(completed.map(|r| r.id));
            return Vec::new();
        }
        let mut out = Vec::new();
        for r in completed {
            if self.reported.contains(&r.id) {
                continue;
            }
            self.reported.insert(r.id);
            let ok = match r.conclusion.as_deref() {
                Some("success") => true,
                Some("failure") | Some("timed_out") | Some("cancelled") => false,
                _ => continue,
            };
            out.push(Event::GithubRun {
                repo: repo.to_string(),
                ok,
            });
        }
        out
    }
}

fn rfc3339(s: Option<&str>) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(s?)
        .ok()
        .map(|d| d.timestamp())
}

/// Runs of one `GET /repos/{repo}/actions/runs` page; `repo` stands in when
/// the payload carries no `repository.full_name`.
pub fn parse_ci_runs(json: &str, repo: &str) -> Result<Vec<CiRun>> {
    let v: Value = serde_json::from_str(json).context("runs json")?;
    let arr = v
        .get("workflow_runs")
        .and_then(Value::as_array)
        .context("workflow_runs missing")?;
    Ok(arr
        .iter()
        .filter_map(|r| {
            let status = match (
                r.get("status").and_then(Value::as_str)?,
                r.get("conclusion").and_then(Value::as_str),
            ) {
                ("completed", Some("success")) => CiStatus::Success,
                ("completed", Some("failure") | Some("timed_out")) => CiStatus::Failure,
                ("completed", _) => CiStatus::Other,
                _ => CiStatus::InProgress,
            };
            let created_at = rfc3339(r.get("created_at").and_then(Value::as_str))?;
            let updated_at =
                rfc3339(r.get("updated_at").and_then(Value::as_str)).unwrap_or(created_at);
            let started = rfc3339(r.get("run_started_at").and_then(Value::as_str));
            let duration_secs = match (status, started) {
                (CiStatus::InProgress, _) | (_, None) => None,
                (_, Some(s)) => Some((updated_at - s).max(0)),
            };
            Some(CiRun {
                id: r.get("id")?.as_u64()?,
                repo: r
                    .pointer("/repository/full_name")
                    .and_then(Value::as_str)
                    .unwrap_or(repo)
                    .to_string(),
                status,
                created_at,
                updated_at,
                duration_secs,
            })
        })
        .collect())
}

/// All repos' runs merged, newest first, the last `CI_PER_PAGE` kept.
pub fn merge_runs(mut all: Vec<CiRun>) -> Vec<CiRun> {
    all.sort_by(|a, b| b.created_at.cmp(&a.created_at).then(b.id.cmp(&a.id)));
    all.truncate(CI_PER_PAGE);
    all
}

/// Non-fork repos pushed in the last 7 days, most recent first, at most
/// `CI_MAX_REPOS`, from `GET /user/repos?sort=pushed`.
pub fn parse_user_repos(json: &str, now_unix: i64) -> Vec<String> {
    let Ok(v) = serde_json::from_str::<Value>(json) else {
        return Vec::new();
    };
    let Some(arr) = v.as_array() else {
        return Vec::new();
    };
    let mut repos: Vec<(i64, String)> = arr
        .iter()
        .filter(|r| !r.get("fork").and_then(Value::as_bool).unwrap_or(false))
        .filter_map(|r| {
            let pushed = rfc3339(r.get("pushed_at").and_then(Value::as_str))?;
            let name = r.get("full_name")?.as_str()?.to_string();
            (now_unix - pushed <= CI_RECENT_PUSH_SECS).then_some((pushed, name))
        })
        .collect();
    repos.sort_by_key(|r| std::cmp::Reverse(r.0));
    repos.truncate(CI_MAX_REPOS);
    repos.into_iter().map(|(_, n)| n).collect()
}

/// One trip to the events feed. GitHub asks for a polling interval and hands
/// out an ETag on a 304 as well, so both survive an unchanged feed.
pub struct EventsPage {
    /// `None` on 304: nothing new since the ETag.
    pub events: Option<Vec<GhEvent>>,
    /// The ETag of a page that actually arrived; only stored when `events` is
    /// `Some`, since a 304 returns the ETag the caller already holds.
    pub etag: Option<String>,
    pub poll_secs: Option<u64>,
}

struct Http {
    token: String,
}

impl Http {
    fn auth(&self, req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        req.header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
    }

    async fn calendar(&self, window: &[NaiveDate]) -> Result<(String, Vec<(String, u32)>)> {
        let (from, to) = calendar_bounds(window, &Local);
        let body = json!({ "query": CALENDAR_QUERY, "variables": { "from": from, "to": to } });
        let resp = self
            .auth(client().post(GRAPHQL))
            .json(&body)
            .send()
            .await
            .context("graphql request")?;
        let status = resp.status();
        let text = resp.text().await.context("graphql body")?;
        if !status.is_success() {
            return Err(HttpStatus {
                status: status.as_u16(),
                body: text.chars().take(120).collect(),
            }
            .into());
        }
        parse_calendar(&text, window)
    }

    /// The feed page, if GitHub sent one; the ETag and `X-Poll-Interval` come
    /// back either way, so a 304 still moves our polling to GitHub's cadence.
    async fn events(&self, login: &str, etag: Option<&str>) -> Result<EventsPage> {
        let mut req = self.auth(client().get(format!("{API}/users/{login}/events?per_page=30")));
        if let Some(tag) = etag {
            req = req.header("If-None-Match", tag);
        }
        let resp = req.send().await.context("events request")?;
        let status = resp.status();
        let poll = resp
            .headers()
            .get("X-Poll-Interval")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse().ok());
        let new_etag = resp
            .headers()
            .get("ETag")
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        if status.as_u16() == 304 {
            return Ok(EventsPage {
                events: None,
                etag: new_etag,
                poll_secs: poll,
            });
        }
        let text = resp.text().await.context("events body")?;
        if !status.is_success() {
            return Err(HttpStatus {
                status: status.as_u16(),
                body: text.chars().take(120).collect(),
            }
            .into());
        }
        Ok(EventsPage {
            events: Some(parse_events(&text)?),
            etag: new_etag,
            poll_secs: poll,
        })
    }

    async fn ci_runs(&self, repo: &str) -> Result<Vec<CiRun>> {
        let resp = self
            .auth(client().get(format!(
                "{API}/repos/{repo}/actions/runs?per_page={CI_PER_PAGE}"
            )))
            .send()
            .await
            .context("ci runs request")?;
        let status = resp.status();
        let text = resp.text().await.context("ci runs body")?;
        if !status.is_success() {
            return Err(HttpStatus {
                status: status.as_u16(),
                body: text.chars().take(120).collect(),
            }
            .into());
        }
        parse_ci_runs(&text, repo)
    }

    async fn user_repos(&self) -> Result<String> {
        let resp = self
            .auth(client().get(format!("{API}/user/repos?sort=pushed&per_page=30")))
            .send()
            .await
            .context("user repos request")?;
        let status = resp.status();
        let text = resp.text().await.context("user repos body")?;
        if !status.is_success() {
            return Err(HttpStatus {
                status: status.as_u16(),
                body: text.chars().take(120).collect(),
            }
            .into());
        }
        Ok(text)
    }

    async fn runs(&self, repo: &str) -> Result<Vec<Run>> {
        let resp = self
            .auth(client().get(format!("{API}/repos/{repo}/actions/runs?per_page=5")))
            .send()
            .await
            .context("runs request")?;
        let status = resp.status();
        let text = resp.text().await.context("runs body")?;
        if !status.is_success() {
            return Err(HttpStatus {
                status: status.as_u16(),
                body: text.chars().take(120).collect(),
            }
            .into());
        }
        parse_runs(&text)
    }
}

pub async fn run_github(cfg: GithubConfig, ctx: SourceCtx) {
    let http = Http {
        token: cfg.token.clone(),
    };
    let mut login: Option<String> = None;
    let mut etag: Option<String> = None;
    let mut events = EventTracker::default();
    let mut runs = RunTracker::default();
    let mut repos: Vec<String> = Vec::new();
    let mut failures = 0u32;
    let mut warned_about_token = false;
    let mut last_calendar: Option<std::time::Instant> = None;
    let mut ci_repos: Vec<String> = cfg.ci_repos.clone();
    let mut ci_discovered_at: Option<std::time::Instant> = None;
    let mut ci_warned: HashMap<String, std::time::Instant> = HashMap::new();
    loop {
        if ctx.shutdown.is_cancelled() {
            return;
        }
        let mut wait = cfg.poll_secs.max(60);
        let step: Result<()> = async {
            let calendar_due =
                last_calendar.is_none_or(|t| t.elapsed() >= Duration::from_secs(CALENDAR_SECS));
            if calendar_due {
                let today = Local::now().date_naive();
                let window = window_days(today);
                let (who, days) = http.calendar(&window).await?;
                tracing::info!(
                    "github: calendar ok ({} today, {} this window)",
                    days.last().map_or(0, |d| d.1),
                    days.iter().map(|d| d.1).sum::<u32>()
                );
                login = Some(who);
                last_calendar = Some(std::time::Instant::now());
                ctx.emit(Event::GithubActivity { days });
            }
            let who = login.as_deref().context("login unknown")?;
            let page = http.events(who, etag.as_deref()).await?;
            // GitHub's cadence holds even when the feed is unchanged
            if let Some(p) = page.poll_secs {
                wait = wait.max(p);
            }
            if let Some(feed) = page.events {
                etag = page.etag;
                repos = pushed_repos(&feed);
                ctx.emit_all(events.diff(&feed));
            }
            for repo in repos.clone() {
                match http.runs(&repo).await {
                    Ok(page) => ctx.emit_all(runs.diff(&repo, &page)),
                    // no `github:` prefix: a per-repo miss does not take the
                    // link down, and the Status dot keys off that needle.
                    Err(e) => tracing::warn!("github runs for {repo}: {e:#}"),
                }
            }
            if cfg.ci_repos.is_empty()
                && ci_discovered_at
                    .is_none_or(|t| t.elapsed() >= Duration::from_secs(CALENDAR_SECS))
            {
                let json = http.user_repos().await?;
                ci_repos = parse_user_repos(&json, chrono::Utc::now().timestamp());
                ci_discovered_at = Some(std::time::Instant::now());
                tracing::info!("github: ci watches {} repos", ci_repos.len());
            }
            let mut all_runs: Vec<CiRun> = Vec::new();
            for repo in &ci_repos {
                match http.ci_runs(repo).await {
                    Ok(runs) => all_runs.extend(runs),
                    // a per-repo miss (404 private, 403 no Actions scope) is
                    // logged once an hour and never takes the link down
                    Err(e) => {
                        let due = ci_warned
                            .get(repo)
                            .is_none_or(|t| t.elapsed() >= Duration::from_secs(CI_WARN_SECS));
                        if due {
                            tracing::warn!("github ci runs for {repo}: {e:#}");
                            ci_warned.insert(repo.clone(), std::time::Instant::now());
                        } else {
                            tracing::debug!("github ci runs for {repo}: {e:#}");
                        }
                    }
                }
            }
            if !ci_repos.is_empty() {
                ctx.emit(Event::CiRuns(merge_runs(all_runs)));
            }
            Ok(())
        }
        .await;
        match step {
            Ok(()) => {
                failures = 0;
                warned_about_token = false;
                ctx.emit(Event::Link {
                    target: LinkTarget::Github,
                    up: true,
                });
            }
            Err(e) => {
                failures += 1;
                let rejection = token_rejected(&e);
                let rejected = rejection.is_some();
                match rejection {
                    Some(status) if !warned_about_token => {
                        warned_about_token = true;
                        tracing::warn!(
                            "github: token rejected (HTTP {status}); check the token in Configure"
                        );
                    }
                    Some(_) => tracing::debug!("github: {e:#}"),
                    None => tracing::warn!("github: {e:#}"),
                }
                if failures >= 2 || rejected {
                    ctx.emit(Event::Link {
                        target: LinkTarget::Github,
                        up: false,
                    });
                }
                wait = backoff_secs(cfg.poll_secs, failures, rejected);
            }
        }
        tokio::select! {
            _ = ctx.shutdown.cancelled() => return,
            _ = tokio::time::sleep(Duration::from_secs(wait)) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    const CAL: &str = include_str!("../tests/fixtures/github-calendar.json");
    const EVENTS: &str = include_str!("../tests/fixtures/github-events.json");
    const RUNS: &str = include_str!("../tests/fixtures/github-runs.json");

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn window_is_thirty_days_ending_today() {
        let w = window_days(d("2026-09-07"));
        assert_eq!(w.len(), 30);
        assert_eq!(w[0], d("2026-08-09"));
        assert_eq!(w[29], d("2026-09-07"));
    }

    #[test]
    fn calendar_bounds_are_local_midnight_with_the_real_offset() {
        let tz = chrono_tz::Europe::Amsterdam;
        let (from, to) = calendar_bounds(&window_days(d("2026-09-07")), &tz);
        assert_eq!(from, "2026-08-09T00:00:00+02:00");
        assert_eq!(to, "2026-09-08T00:00:00+02:00");
        // winter is an hour closer to UTC
        let (from, to) = calendar_bounds(&window_days(d("2026-01-15")), &tz);
        assert_eq!(from, "2025-12-17T00:00:00+01:00");
        assert_eq!(to, "2026-01-16T00:00:00+01:00");
    }

    #[test]
    fn calendar_maps_onto_the_window_with_zeros_for_missing_days() {
        let (login, days) = parse_calendar(CAL, &window_days(d("2026-09-07"))).unwrap();
        assert_eq!(login, "SilkePilon");
        assert_eq!(days.len(), 30);
        assert_eq!(days[29], ("2026-09-07".to_string(), 28));
        assert_eq!(days[28].1, 43);
        assert_eq!(days[27].1, 25);
        assert_eq!(days[0], ("2026-08-09".to_string(), 0));
        assert!(parse_calendar("{\"errors\":[{\"message\":\"Bad credentials\"}]}", &[]).is_err());
    }

    #[test]
    fn events_map_to_kinds_and_skip_the_rest() {
        let evs = parse_events(EVENTS).unwrap();
        assert_eq!(evs.len(), 4, "CreateEvent and the unmerged PR are ignored");
        assert_eq!(evs[0].kind, GhKind::Release("v0.3.0".into()));
        assert_eq!(evs[1].kind, GhKind::Merge);
        assert_eq!(evs[2].kind, GhKind::Star);
        assert_eq!(
            evs[3],
            GhEvent {
                id: "1001".into(),
                repo: "silkepilon/RackScreen".into(),
                kind: GhKind::Push(3),
            }
        );
        assert_eq!(
            pushed_repos(&evs),
            vec!["silkepilon/RackScreen".to_string()]
        );
    }

    #[test]
    fn event_tracker_primes_then_emits_only_new_ids() {
        let evs = parse_events(EVENTS).unwrap();
        let mut t = EventTracker::default();
        assert!(t.diff(&evs).is_empty(), "first page only primes");
        assert!(t.diff(&evs).is_empty());
        let mut newer = vec![GhEvent {
            id: "1006".into(),
            repo: "silkepilon/RackScreen".into(),
            kind: GhKind::Push(1),
        }];
        newer.extend(evs.iter().cloned());
        let out = t.diff(&newer);
        assert_eq!(
            out,
            vec![Event::GithubPush {
                repo: "silkepilon/RackScreen".into(),
                commits: 1,
            }]
        );
        // several new ones arrive oldest first
        let two = vec![
            GhEvent {
                id: "1008".into(),
                repo: "r".into(),
                kind: GhKind::Star,
            },
            GhEvent {
                id: "1007".into(),
                repo: "r".into(),
                kind: GhKind::Merge,
            },
        ];
        let out = t.diff(&two);
        assert_eq!(
            out,
            vec![
                Event::GithubMerge { repo: "r".into() },
                Event::GithubStar { repo: "r".into() }
            ]
        );
    }

    #[test]
    fn runs_parse_and_the_tracker_reports_completions_once() {
        let runs = parse_runs(RUNS).unwrap();
        assert_eq!(runs.len(), 3);
        assert_eq!(
            runs[0],
            Run {
                id: 301,
                status: "in_progress".into(),
                conclusion: None
            }
        );
        let mut t = RunTracker::default();
        assert!(t.diff("r", &runs).is_empty(), "a repo's first page primes");
        let mut done = runs.clone();
        done[0].status = "completed".into();
        done[0].conclusion = Some("success".into());
        assert_eq!(
            t.diff("r", &done),
            vec![Event::GithubRun {
                repo: "r".into(),
                ok: true
            }]
        );
        assert!(t.diff("r", &done).is_empty(), "reported once");
        let mut cancelled = done.clone();
        cancelled.insert(
            0,
            Run {
                id: 302,
                status: "completed".into(),
                conclusion: Some("cancelled".into()),
            },
        );
        assert_eq!(
            t.diff("r", &cancelled),
            vec![Event::GithubRun {
                repo: "r".into(),
                ok: false
            }]
        );
        let mut skipped = cancelled.clone();
        skipped.insert(
            0,
            Run {
                id: 303,
                status: "completed".into(),
                conclusion: Some("skipped".into()),
            },
        );
        assert!(
            t.diff("r", &skipped).is_empty(),
            "skipped is neither pass nor fail"
        );
    }
    const CI_RUNS: &str = include_str!("../tests/fixtures/github-ci-runs.json");
    const USER_REPOS: &str = include_str!("../tests/fixtures/github-user-repos.json");

    #[test]
    fn ci_runs_parse_with_timestamps_and_durations() {
        let runs = parse_ci_runs(CI_RUNS, "fallback/repo").unwrap();
        assert_eq!(runs.len(), 4);
        assert_eq!(runs[0].id, 401);
        assert_eq!(runs[0].repo, "silkepilon/RackScreen");
        assert_eq!(runs[0].status, CiStatus::InProgress);
        assert_eq!(runs[0].duration_secs, None, "still running");
        assert_eq!(runs[1].status, CiStatus::Failure);
        assert_eq!(runs[1].duration_secs, Some(134));
        assert_eq!(runs[2].status, CiStatus::Success);
        assert_eq!(runs[2].duration_secs, Some(108));
        assert_eq!(runs[2].created_at, 1_788_775_200);
        assert_eq!(runs[2].updated_at, 1_788_775_308);
        assert_eq!(runs[3].status, CiStatus::Other, "cancelled");
        assert_eq!(runs[3].duration_secs, None, "no run_started_at");
        let bare = parse_ci_runs(r#"{"workflow_runs":[{"id":1,"status":"queued","conclusion":null,"created_at":"2026-09-07T12:00:00Z","updated_at":"2026-09-07T12:00:00Z"}]}"#, "a/b").unwrap();
        assert_eq!(
            (bare[0].repo.as_str(), bare[0].status),
            ("a/b", CiStatus::InProgress)
        );
        assert!(parse_ci_runs("{}", "a/b").is_err());
    }

    #[test]
    fn runs_merge_newest_first_and_keep_twelve() {
        let mut all: Vec<CiRun> = (0..15)
            .map(|i| CiRun {
                id: i,
                repo: if i % 2 == 0 {
                    "a/b".into()
                } else {
                    "c/d".into()
                },
                status: CiStatus::Success,
                created_at: 1000 + i as i64 * 10,
                updated_at: 1000 + i as i64 * 10 + 5,
                duration_secs: Some(5),
            })
            .collect();
        all.rotate_left(4);
        let merged = merge_runs(all);
        assert_eq!(merged.len(), CI_PER_PAGE);
        assert_eq!(merged[0].id, 14, "newest first");
        assert_eq!(merged[11].id, 3);
    }

    #[test]
    fn user_repos_pushed_this_week_at_most_ten_no_forks() {
        let now = 1_788_782_400; // 2026-09-07 12:00Z
        assert_eq!(
            parse_user_repos(USER_REPOS, now),
            vec![
                "silkepilon/RackScreen".to_string(),
                "silkepilon/homelab-infrastructure".into()
            ]
        );
        let many: Vec<String> = (0..14)
            .map(|i| format!("{{\"full_name\":\"u/r{i}\",\"pushed_at\":\"2026-09-07T11:00:00Z\",\"fork\":false}}"))
            .collect();
        let json = format!("[{}]", many.join(","));
        assert_eq!(parse_user_repos(&json, now).len(), CI_MAX_REPOS);
        assert!(parse_user_repos("nope", now).is_empty());
    }
}
