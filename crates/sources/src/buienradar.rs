//! Buienradar rain nowcast: 24 five-minute slots, two hours ahead, NL and BE.

use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Duration as ChronoDuration, NaiveTime, TimeZone};
use rackscreen_core::event::{Event, LinkTarget};

use crate::electricity::{backoff_secs, HttpStatus};
use crate::http::client;
use crate::SourceCtx;

#[derive(Clone, Debug)]
pub struct RainConfig {
    pub lat: f64,
    pub lon: f64,
    pub poll_secs: u64,
}

/// Fewer lines than this and the feed is broken rather than short.
const MIN_LINES: usize = 12;

pub fn raintext_url(cfg: &RainConfig) -> String {
    format!(
        "https://gpsgadget.buienradar.nl/data/raintext?lat={:.2}&lon={:.2}",
        cfg.lat, cfg.lon
    )
}

/// Buienradar's 0..255 scale: `10^((v - 109) / 32)` mm/h, and 0 is dry.
pub fn mm_per_hour(v: u32) -> f32 {
    if v == 0 {
        0.0
    } else {
        10f32.powf((v as f32 - 109.0) / 32.0)
    }
}

/// Lines `VVV|HH:MM` into `Event::Rain`. The first line's clock time is
/// placed on `now`'s local day, or the day before when it reads later than
/// an hour past `now` (the feed straddling midnight).
pub fn parse_raintext<Tz: TimeZone>(text: &str, now: DateTime<Tz>) -> Result<Event> {
    let mut mm = Vec::new();
    let mut first: Option<NaiveTime> = None;
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let (v, t) = line
            .split_once('|')
            .ok_or_else(|| anyhow!("raintext line without '|': {line:?}"))?;
        let v: u32 = v
            .trim()
            .parse()
            .with_context(|| format!("raintext value {v:?}"))?;
        let t = NaiveTime::parse_from_str(t.trim(), "%H:%M")
            .with_context(|| format!("raintext time {t:?}"))?;
        first.get_or_insert(t);
        mm.push(mm_per_hour(v));
    }
    let first = first.ok_or_else(|| anyhow!("raintext is empty"))?;
    anyhow::ensure!(
        mm.len() >= MIN_LINES,
        "raintext has {} lines, expected at least {MIN_LINES}",
        mm.len()
    );
    let today = now.date_naive();
    // The DST fall-back hour maps to two instants: take the first of them
    // rather than failing the whole poll for an hour once a year.
    let mapped = now.timezone().from_local_datetime(&today.and_time(first));
    let mut from = mapped
        .clone()
        .single()
        .or_else(|| mapped.earliest())
        .ok_or_else(|| anyhow!("no local time for {first}"))?;
    if from > now.clone() + ChronoDuration::hours(1) {
        from -= ChronoDuration::days(1);
    }
    Ok(Event::Rain {
        from: from.timestamp(),
        mm_per_h: mm,
    })
}

async fn fetch(url: &str) -> Result<String> {
    let resp = client().get(url).send().await.context("request")?;
    let status = resp.status();
    let body = resp.text().await.context("body")?;
    if !status.is_success() {
        return Err(HttpStatus {
            status: status.as_u16(),
            body: body.chars().take(120).collect(),
        }
        .into());
    }
    Ok(body)
}

/// Poll the nowcast. Buienradar covers NL and BE only and stamps its slots on
/// the Dutch clock, so the zone is fixed at `Europe/Amsterdam` rather than
/// taken from the Pi: a Pi left on UTC would otherwise decode every slot one
/// or two hours off and read `DRY` forever.
pub async fn run_rain(cfg: RainConfig, ctx: SourceCtx) {
    let url = raintext_url(&cfg);
    let mut failures = 0u32;
    loop {
        if ctx.shutdown.is_cancelled() {
            return;
        }
        match fetch(&url)
            .await
            .and_then(|b| {
                parse_raintext(
                    &b,
                    chrono::Utc::now().with_timezone(&chrono_tz::Europe::Amsterdam),
                )
            })
        {
            Ok(ev) => {
                failures = 0;
                if let Event::Rain { mm_per_h, .. } = &ev {
                    let wet = mm_per_h.iter().filter(|v| **v >= 0.1).count();
                    tracing::info!("rain: poll ok ({wet} wet slots of {})", mm_per_h.len());
                }
                ctx.emit(Event::Link {
                    target: LinkTarget::Rain,
                    up: true,
                });
                ctx.emit(ev);
            }
            Err(e) => {
                failures += 1;
                tracing::warn!("rain: {e:#}");
                if failures >= 2 {
                    ctx.emit(Event::Link {
                        target: LinkTarget::Rain,
                        up: false,
                    });
                }
            }
        }
        let wait = backoff_secs(cfg.poll_secs, failures, false);
        tokio::select! {
            _ = ctx.shutdown.cancelled() => return,
            _ = tokio::time::sleep(Duration::from_secs(wait)) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use chrono_tz::Europe::Amsterdam;

    const TXT: &str = include_str!("../tests/fixtures/buienradar-raintext.txt");

    #[test]
    fn intensity_scale() {
        assert_eq!(mm_per_hour(0), 0.0);
        assert!((mm_per_hour(77) - 0.1).abs() < 0.01);
        assert!((mm_per_hour(109) - 1.0).abs() < 0.01);
        assert!((mm_per_hour(141) - 10.0).abs() < 0.1);
        assert!((mm_per_hour(168) - 69.8).abs() < 1.0);
    }

    #[test]
    fn parses_the_fixture_into_slots_from_the_first_line() {
        let now = Amsterdam.with_ymd_and_hms(2026, 9, 7, 21, 52, 0).unwrap();
        let Event::Rain { from, mm_per_h } = parse_raintext(TXT, now).unwrap() else {
            panic!("rain event");
        };
        assert_eq!(mm_per_h.len(), 24);
        assert_eq!(
            from,
            Amsterdam
                .with_ymd_and_hms(2026, 9, 7, 21, 50, 0)
                .unwrap()
                .timestamp()
        );
        assert_eq!(mm_per_h[0], 0.0);
        assert!((mm_per_h[5] - 0.1).abs() < 0.01);
        assert!((mm_per_h[8] - 69.8).abs() < 1.0);
    }

    #[test]
    fn a_first_line_before_midnight_seen_after_midnight_is_yesterday() {
        let text = "000|23:55\n000|00:00\n000|00:05\n000|00:10\n000|00:15\n000|00:20\n000|00:25\n000|00:30\n000|00:35\n000|00:40\n000|00:45\n000|00:50\n";
        let now = Amsterdam.with_ymd_and_hms(2026, 9, 8, 0, 2, 0).unwrap();
        let Event::Rain { from, .. } = parse_raintext(text, now).unwrap() else {
            panic!()
        };
        assert_eq!(
            from,
            Amsterdam
                .with_ymd_and_hms(2026, 9, 7, 23, 55, 0)
                .unwrap()
                .timestamp()
        );
    }

    #[test]
    fn the_ambiguous_dst_fall_back_hour_still_parses() {
        // 2026-10-25 02:30 Amsterdam happens twice; take the earlier instant.
        let text = "000|02:30\n".to_string() + &"000|02:35\n".repeat(23);
        let now = Amsterdam.with_ymd_and_hms(2026, 10, 25, 4, 0, 0).unwrap();
        let ev = parse_raintext(&text, now).expect("ambiguous local time resolves");
        let Event::Rain { from, .. } = ev else {
            panic!("rain event")
        };
        // the earlier of the two 02:30s is still CEST (+2), i.e. 00:30 UTC
        assert_eq!(from, 1_792_888_200);
    }

    #[test]
    fn short_or_broken_input_is_an_error() {
        let now = Amsterdam.with_ymd_and_hms(2026, 9, 7, 21, 52, 0).unwrap();
        assert!(
            parse_raintext("000|21:50\n000|21:55\n", now).is_err(),
            "fewer than 12 lines"
        );
        assert!(parse_raintext("abc\n".repeat(24).as_str(), now).is_err());
        assert!(parse_raintext("", now).is_err());
    }

    #[test]
    fn url_rounds_the_location_to_two_decimals() {
        let cfg = RainConfig {
            lat: 52.3702,
            lon: 4.8952,
            poll_secs: 300,
        };
        assert_eq!(
            raintext_url(&cfg),
            "https://gpsgadget.buienradar.nl/data/raintext?lat=52.37&lon=4.90"
        );
    }
}
