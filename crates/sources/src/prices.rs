//! Day-ahead electricity prices: EnergyZero (NL, public) or ENTSO-E (EU, token).

use std::time::Duration;

use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDate, NaiveDateTime, TimeZone, Timelike, Utc};
use chrono_tz::Tz;
use rackscreen_core::event::{Event, LinkTarget};
use serde_json::Value;

use crate::http::client;
use crate::SourceCtx;

#[derive(Clone, Debug)]
pub enum PriceSource {
    EnergyZero { include_vat: bool },
    Entsoe { token: String, zone: String },
}

#[derive(Clone, Debug)]
pub struct PriceConfig {
    pub source: PriceSource,
    pub poll_secs: u64,
    pub tz: Tz,
}

/// ENTSO-E bidding-zone EIC codes for the countries Electricity Maps zones map to directly.
pub fn entsoe_zone_for(country: &str) -> Option<&'static str> {
    Some(match country.to_ascii_uppercase().as_str() {
        "NL" => "10YNL----------L",
        "BE" => "10YBE----------2",
        "DE" | "DE-LU" => "10Y1001A1001A82H",
        "FR" => "10YFR-RTE------C",
        "AT" => "10YAT-APG------L",
        "DK-DK1" => "10YDK-1--------W",
        "DK-DK2" => "10YDK-2--------M",
        "ES" => "10YES-REE------0",
        "IT-NO" => "10Y1001A1001A73I",
        "PL" => "10YPL-AREA-----S",
        "SE-SE3" => "10Y1001A1001A46L",
        "NO-NO1" => "10YNO-1--------2",
        "FI" => "10YFI-1--------U",
        "CH" => "10YCH-SWISSGRIDZ",
        "CZ" => "10YCZ-CEPS-----N",
        "PT" => "10YPT-REN------W",
        _ => return None,
    })
}

/// Parse a UTC timestamp: RFC 3339, or the seconds-less `2026-09-05T22:00Z` ENTSO-E uses.
fn parse_utc(s: &str) -> Option<DateTime<Utc>> {
    let s = s.trim();
    if let Ok(d) = DateTime::parse_from_rfc3339(&s.replace('Z', "+00:00")) {
        return Some(d.with_timezone(&Utc));
    }
    NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%MZ")
        .ok()
        .map(|n| Utc.from_utc_datetime(&n))
}

/// EnergyZero JSON -> (ISO timestamp, €/kWh).
pub fn parse_energyzero(json: &str) -> Result<Vec<(String, f32)>> {
    let v: Value = serde_json::from_str(json).context("energyzero json")?;
    let arr = v
        .get("Prices")
        .and_then(|p| p.as_array())
        .context("Prices missing")?;
    Ok(arr
        .iter()
        .filter_map(|p| {
            Some((
                p.get("readingDate")?.as_str()?.to_string(),
                p.get("price")?.as_f64()? as f32,
            ))
        })
        .collect())
}

/// ENTSO-E Publication_MarketDocument -> (ISO timestamp, €/MWh) for every hourly Point.
pub fn parse_entsoe(xml: &str) -> Result<Vec<(String, f32)>> {
    use quick_xml::events::Event as X;
    use quick_xml::Reader;
    let mut reader = Reader::from_str(xml);
    let mut out = Vec::new();
    let mut path: Vec<String> = Vec::new();
    let mut start: Option<DateTime<Utc>> = None;
    let mut resolution_min: i64 = 60;
    let mut position: i64 = 0;
    let mut text = String::new();
    loop {
        match reader.read_event().context("xml")? {
            X::Start(e) => {
                path.push(e.name().as_ref().to_string());
                text.clear();
            }
            X::Text(t) => text = t.xml10_content().into_owned(),
            X::End(_) => {
                let tag = path.pop().unwrap_or_default();
                let parent = path.last().map(String::as_str).unwrap_or("");
                match (parent, tag.as_str()) {
                    ("timeInterval", "start") => start = parse_utc(&text),
                    ("Period", "resolution") => {
                        resolution_min = match text.as_str() {
                            "PT15M" => 15,
                            "PT30M" => 30,
                            _ => 60,
                        }
                    }
                    ("Point", "position") => position = text.trim().parse().unwrap_or(0),
                    ("Point", "price.amount") => {
                        if let (Some(s), Ok(p)) = (start, text.trim().parse::<f32>()) {
                            let ts = s + chrono::Duration::minutes((position - 1) * resolution_min);
                            out.push((ts.to_rfc3339(), p));
                        }
                    }
                    _ => {}
                }
                text.clear();
            }
            X::Eof => break,
            _ => {}
        }
    }
    anyhow::ensure!(!out.is_empty(), "no price points in ENTSO-E document");
    Ok(out)
}

/// Bucket timestamped prices into the 24 local hours of `day`, converting to ct/kWh.
/// `per_kwh` = input is €/kWh (EnergyZero) else €/MWh (ENTSO-E). Missing hours are NaN;
/// sub-hourly points average into their hour.
pub fn hourly_ct_for_day(
    points: &[(String, f32)],
    day: NaiveDate,
    tz: Tz,
    per_kwh: bool,
) -> Vec<f32> {
    let mut sum = [0.0f32; 24];
    let mut cnt = [0u32; 24];
    for (ts, price) in points {
        let Some(t) = parse_utc(ts) else { continue };
        let local = t.with_timezone(&tz);
        if local.date_naive() != day {
            continue;
        }
        let h = local.hour() as usize;
        sum[h] += if per_kwh { price * 100.0 } else { price / 10.0 };
        cnt[h] += 1;
    }
    (0..24)
        .map(|h| {
            if cnt[h] == 0 {
                f32::NAN
            } else {
                sum[h] / cnt[h] as f32
            }
        })
        .collect()
}

fn day_bounds_utc(day: NaiveDate, tz: Tz) -> (DateTime<Utc>, DateTime<Utc>) {
    let midnight = day.and_hms_opt(0, 0, 0).unwrap();
    let start = tz
        .from_local_datetime(&midnight)
        .single()
        .unwrap_or_else(|| tz.from_utc_datetime(&midnight));
    let end = start + chrono::Duration::days(1);
    (start.with_timezone(&Utc), end.with_timezone(&Utc))
}

async fn fetch_day(cfg: &PriceConfig, day: NaiveDate) -> Result<Vec<f32>> {
    let (from, till) = day_bounds_utc(day, cfg.tz);
    match &cfg.source {
        PriceSource::EnergyZero { include_vat } => {
            let url = format!(
                "https://api.energyzero.nl/v1/energyprices?fromDate={}&tillDate={}&interval=4&usageType=1&inclBtw={}",
                from.format("%Y-%m-%dT%H:%M:%S%.3fZ"),
                (till - chrono::Duration::milliseconds(1)).format("%Y-%m-%dT%H:%M:%S%.3fZ"),
                include_vat
            );
            let body = client()
                .get(&url)
                .send()
                .await
                .context("energyzero request")?
                .error_for_status()
                .context("energyzero status")?
                .text()
                .await?;
            Ok(hourly_ct_for_day(
                &parse_energyzero(&body)?,
                day,
                cfg.tz,
                true,
            ))
        }
        PriceSource::Entsoe { token, zone } => {
            let url = format!(
                "https://web-api.tp.entsoe.eu/api?securityToken={token}&documentType=A44&in_Domain={zone}&out_Domain={zone}&periodStart={}&periodEnd={}",
                from.format("%Y%m%d%H%M"),
                till.format("%Y%m%d%H%M")
            );
            let body = client()
                .get(&url)
                .send()
                .await
                .context("entsoe request")?
                .error_for_status()
                .context("entsoe status")?
                .text()
                .await?;
            Ok(hourly_ct_for_day(&parse_entsoe(&body)?, day, cfg.tz, false))
        }
    }
}

pub async fn run_prices(cfg: PriceConfig, ctx: SourceCtx) {
    let mut failures = 0u32;
    loop {
        if ctx.shutdown.is_cancelled() {
            return;
        }
        let today = Utc::now().with_timezone(&cfg.tz).date_naive();
        match fetch_day(&cfg, today).await {
            Ok(ct) => {
                failures = 0;
                tracing::info!(
                    "prices: {} hours for {}",
                    ct.iter().filter(|v| v.is_finite()).count(),
                    today
                );
                ctx.emit(Event::Link {
                    target: LinkTarget::Prices,
                    up: true,
                });
                ctx.emit(Event::Prices {
                    date: today.to_string(),
                    ct_per_kwh: ct,
                    currency: "EUR".into(),
                });
            }
            Err(e) => {
                failures += 1;
                tracing::warn!("prices: {e:#}");
                if failures >= 2 {
                    ctx.emit(Event::Link {
                        target: LinkTarget::Prices,
                        up: false,
                    });
                }
            }
        }
        // re-poll at the top of the next hour at the latest so the current-hour marker moves
        let now_local = Utc::now().with_timezone(&cfg.tz);
        let to_next_hour = 3600 - (now_local.minute() * 60 + now_local.second()) as u64 + 5;
        let normal = cfg.poll_secs.max(60);
        let wait = if failures > 0 {
            120
        } else {
            normal.min(to_next_hour)
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

    const EZ: &str = include_str!("../tests/fixtures/energyzero.json");
    const ENTSOE: &str = include_str!("../tests/fixtures/entsoe.xml");

    #[test]
    fn energyzero_to_local_hours() {
        let pts = parse_energyzero(EZ).unwrap();
        assert_eq!(pts.len(), 24);
        let day = NaiveDate::from_ymd_opt(2026, 9, 6).unwrap();
        let ct = hourly_ct_for_day(&pts, day, chrono_tz::Europe::Amsterdam, true);
        assert_eq!(ct.len(), 24);
        // fixture: 22:00Z on the 5th is 00:00 local on the 6th and costs 0.22 €/kWh
        assert!((ct[0] - 22.0).abs() < 0.01);
        assert!(ct[23].is_finite());
    }

    #[test]
    fn entsoe_points_and_units() {
        let pts = parse_entsoe(ENTSOE).unwrap();
        assert_eq!(pts.len(), 24);
        assert_eq!(pts[0].1, 85.5);
        let day = NaiveDate::from_ymd_opt(2026, 9, 6).unwrap();
        let ct = hourly_ct_for_day(&pts, day, chrono_tz::Europe::Amsterdam, false);
        assert!((ct[0] - 8.55).abs() < 0.01, "€/MWh to ct/kWh");
        assert!(parse_entsoe("<x/>").is_err());
    }

    #[test]
    fn missing_hours_are_nan_and_zones_map() {
        let day = NaiveDate::from_ymd_opt(2026, 9, 6).unwrap();
        let ct = hourly_ct_for_day(
            &[("2026-09-06T10:00:00Z".into(), 0.1)],
            day,
            chrono_tz::Europe::Amsterdam,
            true,
        );
        assert!(ct[12].is_finite() && ct[11].is_nan());
        assert_eq!(entsoe_zone_for("nl"), Some("10YNL----------L"));
        assert_eq!(entsoe_zone_for("XX"), None);
    }
}
