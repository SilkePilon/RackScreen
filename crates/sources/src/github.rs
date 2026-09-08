//! GitHub poller: the contribution calendar (GraphQL), the public events feed
//! for splashes, and Actions runs for the repos pushed to lately.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use chrono::{Local, NaiveDate};
use rackscreen_core::event::{Event, LinkTarget};
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

#[derive(Clone, Debug)]
pub struct GithubConfig {
    pub token: String,
    pub poll_secs: u64,
}

/// Thirty dates ending on `today`, oldest first.
pub fn window_days(today: NaiveDate) -> Vec<NaiveDate> {
    (0..WINDOW_DAYS)
        .rev()
        .map(|back| today - chrono::Duration::days(back))
        .collect()
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
        let from = format!("{}T00:00:00Z", window[0].format("%Y-%m-%d"));
        let to = format!(
            "{}T00:00:00Z",
            (window[window.len() - 1] + chrono::Duration::days(1)).format("%Y-%m-%d")
        );
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

    /// `Ok(None)` on 304 (nothing new); otherwise the page and the new ETag.
    async fn events(
        &self,
        login: &str,
        etag: Option<&str>,
    ) -> Result<Option<(Vec<GhEvent>, Option<String>, Option<u64>)>> {
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
            return Ok(None);
        }
        let text = resp.text().await.context("events body")?;
        if !status.is_success() {
            return Err(HttpStatus {
                status: status.as_u16(),
                body: text.chars().take(120).collect(),
            }
            .into());
        }
        Ok(Some((parse_events(&text)?, new_etag, poll)))
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
            if let Some((page, new_etag, poll)) = http.events(who, etag.as_deref()).await? {
                etag = new_etag;
                if let Some(p) = poll {
                    wait = wait.max(p);
                }
                repos = pushed_repos(&page);
                ctx.emit_all(events.diff(&page));
            }
            for repo in repos.clone() {
                match http.runs(&repo).await {
                    Ok(page) => ctx.emit_all(runs.diff(&repo, &page)),
                    Err(e) => tracing::warn!("github: runs for {repo}: {e:#}"),
                }
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
                let rejected = token_rejected(&e).is_some();
                match token_rejected(&e) {
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
}
