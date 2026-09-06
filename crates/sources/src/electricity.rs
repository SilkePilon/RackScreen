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
    anyhow::ensure!(
        status.is_success(),
        "electricity maps HTTP {status}: {}",
        body.chars().take(120).collect::<String>()
    );
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
    loop {
        if ctx.shutdown.is_cancelled() {
            return;
        }
        match poll_once(&cfg).await {
            Ok(ev) => {
                failures = 0;
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
                tracing::warn!("electricity: {e:#}");
                if failures >= 2 {
                    ctx.emit(Event::Link {
                        target: LinkTarget::Electricity,
                        up: false,
                    });
                }
            }
        }
        let wait = if failures > 0 {
            60
        } else {
            cfg.poll_secs.max(60)
        };
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
    fn parses_carbon() {
        assert_eq!(parse_carbon(CI).unwrap(), 214.0);
        assert!(parse_carbon("{}").is_err());
    }
}
