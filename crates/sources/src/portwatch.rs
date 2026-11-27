//! PortWatch VI tracker: `tracker.json` from portwatch-vi.xyz, scored with
//! the site's formula (`rackscreen_core::portwatch`) into the leader snapshot.

use std::time::Duration;

use anyhow::{bail, Context, Result};
use rackscreen_core::event::Event;
use rackscreen_core::portwatch::{
    parse_gpus, snapshot, stability_label, Effort, PortWatchSnapshot,
};
use serde::Deserialize;
use serde_json::Value;

use crate::http::{client, read_capped};
use crate::SourceCtx;

pub const URL: &str = "https://portwatch-vi.xyz/data/tracker.json";
/// The file is ~320 KB; refuse anything absurd.
const MAX_BYTES: usize = 4 * 1024 * 1024;
/// The leader's name is the site's untrusted text: at most this many chars.
const MAX_NAME_CHARS: usize = 40;

#[derive(Clone, Debug)]
pub struct PortWatchConfig {
    pub poll_secs: u64,
}

#[derive(Deserialize)]
struct Tracker {
    updated: String,
    #[serde(default)]
    efforts: Vec<RawEffort>,
}

/// Only the fields the screen needs; every one may be missing or odd.
#[derive(Deserialize, Default)]
#[serde(default)]
struct RawEffort {
    id: Value,
    name: Value,
    repo: Value,
    verification: Value,
    gpu: Value,
    gta: Value,
    groundwork: Value,
    #[serde(rename = "manualScore")]
    manual_score: Value,
    nuked: Value,
    stats: Value,
}

/// A JS-style number in 0..=1 terms: `true` 1, a number or numeric string
/// as is (clamped later), anything else 0.
fn num(v: Option<&Value>) -> f64 {
    match v {
        Some(Value::Bool(true)) => 1.0,
        Some(Value::Number(n)) => n.as_f64().unwrap_or(0.0),
        Some(Value::String(s)) => s.trim().parse().unwrap_or(0.0),
        _ => 0.0,
    }
}

/// JavaScript truthiness (`nuked` is an object with a date and a note).
fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0 && !f.is_nan()),
        Value::String(s) => !s.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

/// A display string from untrusted JSON: control characters dropped,
/// trimmed and capped to `MAX_NAME_CHARS`; empty when not a string.
fn clean(v: &Value) -> String {
    let s: String = v
        .as_str()
        .unwrap_or("")
        .chars()
        .filter(|c| !c.is_control())
        .collect();
    let capped: String = s.trim().chars().take(MAX_NAME_CHARS).collect();
    capped.trim_end().to_string()
}

fn effort(raw: &RawEffort) -> Effort {
    let gta = raw.gta.as_object();
    let g = |k: &str| num(gta.and_then(|m| m.get(k)));
    let gw = raw.groundwork.as_object();
    let w = |k: &str| num(gw.and_then(|m| m.get(k)));
    // a number is the owner's rating; a string is a label (a numeric
    // string is a label too, as on the site)
    let stability = match gta.and_then(|m| m.get("stability")) {
        Some(Value::Number(n)) => n.as_f64().unwrap_or(0.0),
        Some(Value::String(s)) => stability_label(s),
        _ => 0.0,
    };
    let gpus = raw
        .gpu
        .as_array()
        .map(|a| parse_gpus(a.iter().filter_map(Value::as_str)))
        .unwrap_or_default();
    Effort {
        name: Some(clean(&raw.name))
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| clean(&raw.id)),
        repo: raw.repo.as_str().map(str::to_string),
        verified: raw.verification.as_str() == Some("portwatch"),
        gpus,
        groundwork: [w("loader"), w("shaders"), w("graphics"), w("proven")],
        ladder: [g("loads"), g("boots"), g("menu"), g("world"), g("playable")],
        story: g("story"),
        render: g("render"),
        fps: g("fps"),
        stability,
        manual_score: raw.manual_score.as_f64(),
        nuked: truthy(&raw.nuked),
        stars: raw.stats.get("stars").and_then(Value::as_u64).unwrap_or(0),
    }
}

pub fn parse_efforts(body: &[u8]) -> Result<(Vec<Effort>, i64)> {
    let t: Tracker = serde_json::from_slice(body).context("tracker.json")?;
    let updated = chrono::DateTime::parse_from_rfc3339(&t.updated)
        .with_context(|| format!("updated {:?}", t.updated))?
        .timestamp();
    Ok((t.efforts.iter().map(effort).collect(), updated))
}

pub fn parse(body: &[u8]) -> Result<PortWatchSnapshot> {
    let (efforts, updated) = parse_efforts(body)?;
    match snapshot(&efforts, updated) {
        Some(s) => Ok(s),
        None => bail!("tracker.json has no ranked efforts"),
    }
}

pub enum Fetched {
    NotModified,
    Body { body: Vec<u8>, etag: Option<String> },
}

/// One GET, conditional on `etag`.
pub async fn fetch(etag: Option<&str>) -> Result<Fetched> {
    use reqwest::header::{ETAG, IF_NONE_MATCH};
    let mut req = client().get(URL);
    if let Some(tag) = etag {
        req = req.header(IF_NONE_MATCH, tag);
    }
    let resp = req.send().await.context("request")?;
    let status = resp.status();
    if status == reqwest::StatusCode::NOT_MODIFIED {
        return Ok(Fetched::NotModified);
    }
    if !status.is_success() {
        bail!("HTTP {}", status.as_u16());
    }
    let etag = resp
        .headers()
        .get(ETAG)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let body = read_capped(resp, MAX_BYTES, "tracker.json").await?;
    Ok(Fetched::Body { body, etag })
}

/// Poll the tracker: a changed file is parsed and sent; an unchanged one
/// (304) costs one request; failures keep the last snapshot on screen and
/// retry after 1, 5, 15, then 60 minutes.
pub async fn run_portwatch(cfg: PortWatchConfig, ctx: SourceCtx) {
    let mut etag: Option<String> = None;
    let mut failures = 0u32;
    loop {
        if ctx.shutdown.is_cancelled() {
            return;
        }
        let got = match fetch(etag.as_deref()).await {
            Ok(Fetched::NotModified) => Ok(None),
            Ok(Fetched::Body { body, etag: tag }) => parse(&body).map(|s| Some((s, tag))),
            Err(e) => Err(e),
        };
        let wait = match got {
            Ok(None) => {
                failures = 0;
                Duration::from_secs(cfg.poll_secs)
            }
            Ok(Some((snap, tag))) => {
                failures = 0;
                etag = tag;
                tracing::info!(
                    "portwatch: leader {} {} % ({})",
                    snap.name,
                    snap.score,
                    snap.milestone.label()
                );
                ctx.emit(Event::PortWatch(snap));
                Duration::from_secs(cfg.poll_secs)
            }
            Err(e) => {
                failures += 1;
                if failures == 1 {
                    tracing::warn!("portwatch: {e:#}");
                } else {
                    tracing::debug!("portwatch: {e:#} (failure {failures})");
                }
                crate::gif::retry_delay(failures)
            }
        };
        tokio::select! {
            _ = ctx.shutdown.cancelled() => return,
            _ = tokio::time::sleep(wait) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rackscreen_core::portwatch::{ranked, top3_average, Gpu, Milestone};

    const FIXTURE: &[u8] = include_bytes!("../testdata/portwatch-tracker.json");

    #[test]
    fn the_fixture_leader_matches_the_site() {
        let (efforts, updated) = parse_efforts(FIXTURE).unwrap();
        assert_eq!(efforts.len(), 45);
        // 2026-11-27T16:18:23.809Z
        assert_eq!(updated, 1_795_796_303);
        let r = ranked(&efforts);
        assert_eq!(efforts[r[0].index].name, "Belobug");
        assert_eq!(r[0].scored.score, 91.3);
        assert_eq!(efforts[r[1].index].name, "VIctory-NVIDIA");
        assert_eq!(r[1].scored.score, 90.0);
        // the site's history entry for the day: top3 90.3
        assert_eq!(top3_average(&r), 90.3);
    }

    #[test]
    fn the_fixture_snapshot() {
        let s = parse(FIXTURE).unwrap();
        assert_eq!(s.name, "Belobug");
        assert_eq!(s.score, 91.3);
        assert_eq!(s.milestone, Milestone::Playable);
        assert!(!s.private);
        assert!(s.verified);
        assert_eq!(s.fps, Some(22.0));
        assert_eq!(s.gpus, vec![Gpu::Amd]);
    }

    #[test]
    fn known_efforts_score_like_the_site() {
        let (efforts, _) = parse_efforts(FIXTURE).unwrap();
        let by = |name: &str| {
            let e = efforts.iter().find(|e| e.name == name).unwrap();
            (rackscreen_core::portwatch::score(e), e)
        };
        let (s, e) = by("Doomsday");
        assert_eq!(s.score, 82.1);
        assert!(e.private() && e.verified);
        assert_eq!(e.gpus, vec![Gpu::Amd, Gpu::Nvidia]);
        let (s, _) = by("XCL");
        assert_eq!((s.score, s.milestone), (35.0, Milestone::MainMenu));
        let (s, _) = by("ScarlettX");
        assert_eq!((s.score, s.milestone), (5.2, Milestone::Groundwork));
    }

    #[test]
    fn nuked_objects_are_skipped() {
        let (efforts, _) = parse_efforts(FIXTURE).unwrap();
        let nuked: Vec<&str> = efforts
            .iter()
            .filter(|e| e.nuked)
            .map(|e| e.name.as_str())
            .collect();
        assert_eq!(nuked.len(), 4, "{nuked:?}");
        assert!(nuked.contains(&"ZETTROX"));
    }

    #[test]
    fn loose_values() {
        let body = br#"{"updated":"2026-11-27T00:00:00Z","efforts":[
          {"name":"A","repo":"a/b","verification":"portwatch","gpu":["NVIDIA",3,"amd"],
           "gta":{"loads":true,"boots":"1","menu":false,"stability":"rarely","fps":"9"},
           "stats":{"stars":7}},
          {"name":"B","repo":null,"gta":{"loads":1,"boots":1,"menu":1,"world":1,"playable":1,
           "story":0.5,"render":0.5,"fps":15,"stability":0.6},"manualScore":12.5,
           "nuked":false},
          {"gta":null,"groundwork":null,"stats":null}
        ]}"#;
        let (e, _) = parse_efforts(body).unwrap();
        assert_eq!(e[0].ladder, [1.0, 1.0, 0.0, 0.0, 0.0]);
        assert_eq!(e[0].stability, 0.8);
        assert_eq!(e[0].fps, 9.0);
        assert_eq!(e[0].gpus, vec![Gpu::Nvidia, Gpu::Amd]);
        assert_eq!(e[0].stars, 7);
        assert!(e[0].verified);
        assert_eq!(e[1].stability, 0.6);
        assert_eq!(e[1].manual_score, Some(12.5));
        assert!(!e[1].nuked && e[1].private());
        assert_eq!(e[2], Effort::default());
    }

    fn names_of(efforts: &str) -> Vec<String> {
        let body = format!(r#"{{"updated":"2026-11-27T00:00:00Z","efforts":[{efforts}]}}"#);
        let (e, _) = parse_efforts(body.as_bytes()).unwrap();
        e.into_iter().map(|e| e.name).collect()
    }

    #[test]
    fn a_long_name_is_capped() {
        let long = "x".repeat(10_000);
        let names = names_of(&format!(r#"{{"id":"a","name":"{long}"}}"#));
        assert_eq!(names[0].chars().count(), 40);
        assert_eq!(names[0], "x".repeat(40));
    }

    #[test]
    fn control_characters_are_dropped() {
        let names = names_of(r#"{"id":"a","name":"  Be\u0007lo\nbug\u001b[2J\t "}"#);
        assert_eq!(names[0], "Belobug[2J");
    }

    #[test]
    fn an_empty_name_falls_back_to_the_id() {
        let names = names_of(
            r#"{"id":"belobug","name":"  "},{"id":"x\u0000y","name":null},
               {"id":"z"},{"name":"\u0007"}"#,
        );
        assert_eq!(names, vec!["belobug", "xy", "z", ""]);
    }

    #[test]
    fn bad_input_is_an_error() {
        assert!(parse(b"not json").is_err());
        assert!(parse(br#"{"updated":"2026-11-27T00:00:00Z","efforts":[]}"#).is_err());
        assert!(parse(br#"{"updated":"yesterday","efforts":[{"name":"A"}]}"#).is_err());
    }

    /// Hits the real site: `cargo test -p rackscreen-sources portwatch_live -- --ignored`.
    #[tokio::test]
    #[ignore]
    async fn portwatch_live() {
        match fetch(None).await.unwrap() {
            Fetched::Body { body, etag } => {
                let s = parse(&body).unwrap();
                eprintln!("leader {} {} % (etag {etag:?})", s.name, s.score);
                assert!(s.score > 0.0);
            }
            Fetched::NotModified => panic!("no etag was sent"),
        }
    }
}
