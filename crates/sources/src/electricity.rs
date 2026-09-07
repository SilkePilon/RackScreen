//! Electricity Maps API poller: production mix, renewable / fossil-free share, carbon intensity.

use std::time::Duration;

use anyhow::{Context, Result};
use rackscreen_core::electricity::Source;
use rackscreen_core::event::{Event, LinkTarget};
use serde_json::Value;

use crate::http::client;
use crate::SourceCtx;

const BASE: &str = "https://api.electricitymap.org/v3";

#[derive(Clone, Debug)]
pub struct ElectricityConfig {
    pub zone: String,
    pub token: String,
    pub poll_secs: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PowerBreakdown {
    pub mix_mw: Vec<(Source, f32)>,
    pub renewable_pct: f32,
    pub fossil_free_pct: f32,
    pub datetime: String,
}

pub fn parse_power_breakdown(json: &str) -> Result<PowerBreakdown> {
    let v: Value = serde_json::from_str(json).context("power-breakdown json")?;
    let prod = v
        .get("powerProductionBreakdown")
        .and_then(|p| p.as_object())
        .context("powerProductionBreakdown missing")?;
    let mut mix_mw = Vec::new();
    for (k, val) in prod {
        let Some(src) = Source::from_api_key(k) else {
            continue;
        };
        let mw = val.as_f64().unwrap_or(0.0) as f32;
        if mw > 0.0 {
            mix_mw.push((src, mw));
        }
    }
    let pct = |key: &str| v.get(key).and_then(|x| x.as_f64()).unwrap_or(0.0) as f32;
    Ok(PowerBreakdown {
        mix_mw,
        renewable_pct: pct("renewablePercentage"),
        fossil_free_pct: pct("fossilFreePercentage"),
        datetime: v
            .get("datetime")
            .and_then(|d| d.as_str())
            .unwrap_or("")
            .to_string(),
    })
}

pub fn parse_carbon(json: &str) -> Result<f32> {
    let v: Value = serde_json::from_str(json).context("carbon-intensity json")?;
    v.get("carbonIntensity")
        .and_then(|c| c.as_f64())
        .map(|c| c as f32)
        .context("carbonIntensity missing")
}

/// A non-2xx answer from Electricity Maps, kept typed so the poller can tell a
/// rejected token from a network hiccup.
#[derive(Debug, thiserror::Error)]
#[error("electricity maps HTTP {status}: {body}")]
pub struct HttpStatus {
    pub status: u16,
    pub body: String,
}

/// 401/403: the token is wrong, disabled or out of quota. Retrying faster will
/// not fix it, and the user has to edit the config.
fn token_rejected(e: &anyhow::Error) -> Option<u16> {
    e.downcast_ref::<HttpStatus>()
        .map(|h| h.status)
        .filter(|s| *s == 401 || *s == 403)
}

/// Seconds until the next poll. One quick retry covers a hiccup; once the link
/// is down, or the token has been rejected, back off to the normal interval
/// instead of hammering the API.
fn backoff_secs(poll_secs: u64, failures: u32, rejected: bool) -> u64 {
    let normal = poll_secs.max(60);
    if failures == 1 && !rejected {
        60.min(normal)
    } else {
        normal
    }
}

async fn fetch(path: &str, zone: &str, token: &str) -> Result<String> {
    let url = format!("{BASE}/{path}/latest?zone={zone}");
    let resp = client()
        .get(&url)
        .header("auth-token", token)
        .send()
        .await
        .context("request")?;
    let status = resp.status();
    let body = resp.text().await.context("body")?;
    if !status.is_success() {
        return Err(HttpStatus {
            status: status.as_u16(),
            body: body.chars().take(120).collect::<String>(),
        }
        .into());
    }
    Ok(body)
}

pub async fn poll_once(cfg: &ElectricityConfig) -> Result<Event> {
    let pb = parse_power_breakdown(&fetch("power-breakdown", &cfg.zone, &cfg.token).await?)?;
    let carbon = parse_carbon(&fetch("carbon-intensity", &cfg.zone, &cfg.token).await?)?;
    Ok(Event::Electricity {
        zone: cfg.zone.clone(),
        mix_mw: pb.mix_mw,
        renewable_pct: pb.renewable_pct,
        fossil_free_pct: pb.fossil_free_pct,
        carbon_gco2: carbon,
        updated_at: pb.datetime,
    })
}

pub async fn run_electricity(cfg: ElectricityConfig, ctx: SourceCtx) {
    let mut failures = 0u32;
    let mut rejected;
    let mut warned_about_token = false;
    loop {
        if ctx.shutdown.is_cancelled() {
            return;
        }
        match poll_once(&cfg).await {
            Ok(ev) => {
                failures = 0;
                rejected = false;
                warned_about_token = false;
                if let Event::Electricity { mix_mw, .. } = &ev {
                    tracing::info!("electricity: poll ok ({} sources)", mix_mw.len());
                }
                ctx.emit(Event::Link {
                    target: LinkTarget::Electricity,
                    up: true,
                });
                ctx.emit(ev);
            }
            Err(e) => {
                failures += 1;
                match token_rejected(&e) {
                    // a configuration problem: say so once, then stay quiet
                    Some(status) if !warned_about_token => {
                        warned_about_token = true;
                        tracing::warn!(
                            "electricity: token rejected (HTTP {status}); check the token in Configure"
                        );
                    }
                    Some(_) => tracing::debug!("electricity: {e:#}"),
                    None => tracing::warn!("electricity: {e:#}"),
                }
                rejected = token_rejected(&e).is_some();
                if failures >= 2 || rejected {
                    ctx.emit(Event::Link {
                        target: LinkTarget::Electricity,
                        up: false,
                    });
                }
            }
        }
        let wait = backoff_secs(cfg.poll_secs, failures, rejected);
        tokio::select! {
            _ = ctx.shutdown.cancelled() => return,
            _ = tokio::time::sleep(Duration::from_secs(wait)) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PB: &str = include_str!("../tests/fixtures/em-power-breakdown.json");
    const CI: &str = include_str!("../tests/fixtures/em-carbon.json");

    #[test]
    fn parses_breakdown_with_storage_and_nulls() {
        let pb = parse_power_breakdown(PB).unwrap();
        assert!(pb
            .mix_mw
            .iter()
            .any(|(s, mw)| *s == Source::Solar && *mw > 1000.0));
        assert!(pb.mix_mw.iter().any(|(s, _)| *s == Source::HydroStorage));
        assert!(
            !pb.mix_mw.iter().any(|(s, _)| *s == Source::Oil),
            "null and zero omitted"
        );
        assert!((pb.renewable_pct - 61.0).abs() < 0.01);
        assert!((pb.fossil_free_pct - 73.0).abs() < 0.01);
        assert_eq!(pb.datetime, "2026-09-06T12:00:00.000Z");
    }

    #[test]
    fn a_rejected_token_backs_off_like_a_dead_link() {
        let unauthorized = |status| {
            anyhow::Error::new(HttpStatus {
                status,
                body: "invalid token".into(),
            })
        };
        assert_eq!(token_rejected(&unauthorized(401)), Some(401));
        assert_eq!(token_rejected(&unauthorized(403)), Some(403));
        assert_eq!(token_rejected(&unauthorized(500)), None);
        assert_eq!(token_rejected(&anyhow::anyhow!("timed out")), None);
        // healthy: the configured interval, never below a minute
        assert_eq!(backoff_secs(300, 0, false), 300);
        assert_eq!(backoff_secs(10, 0, false), 60);
        // one quick retry, then the normal interval instead of a minute
        assert_eq!(backoff_secs(300, 1, false), 60);
        assert_eq!(backoff_secs(300, 2, false), 300);
        assert_eq!(backoff_secs(300, 9, false), 300);
        // a rejected token never gets the quick retry
        assert_eq!(backoff_secs(300, 1, true), 300);
    }

    #[test]
    fn parses_carbon() {
        assert_eq!(parse_carbon(CI).unwrap(), 214.0);
        assert!(parse_carbon("{}").is_err());
    }
}
