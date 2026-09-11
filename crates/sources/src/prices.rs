//! Day-ahead electricity prices: EnergyZero (NL, public) or ENTSO-E (EU, token).

use std::time::Duration;

use anyhow::{Context, Result};
use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, TimeZone, Timelike, Utc};
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
    /// The system local zone, the same clock `runloop` uses for the
    /// current-hour marker; set the Pi's zone with `timedatectl`.
    pub tz: Local,
}

const ENTSOE_URL: &str = "https://web-api.tp.entsoe.eu/api";

#[allow(dead_code)]
const ENERGY_CHARTS_URL: &str = "https://api.energy-charts.info/v2/price";
/// Quarter-hours in a local day; the day-ahead market's time unit.
pub const SLOTS_PER_DAY: usize = 96;

/// Energy-Charts `/v2/price` (schema 2.0) -> (UTC time, €/MWh), one per
/// quarter-hour. Entries without a timestamp or with a null price are skipped.
pub fn parse_energy_charts(json: &str) -> Result<Vec<(DateTime<Utc>, f32)>> {
    let v: Value = serde_json::from_str(json).context("energy-charts json")?;
    let data = v
        .get("data")
        .and_then(Value::as_array)
        .context("data missing")?;
    let out: Vec<(DateTime<Utc>, f32)> = data
        .iter()
        .filter_map(|p| {
            let ts = p.get("timestamp")?.as_str()?;
            let t = DateTime::parse_from_rfc3339(ts).ok()?.with_timezone(&Utc);
            let price = p.get("values")?.get("day_ahead_price")?.as_f64()? as f32;
            Some((t, price))
        })
        .collect();
    anyhow::ensure!(!out.is_empty(), "no price points in Energy-Charts document");
    Ok(out)
}

/// The exchange price per MWh as a consumer price per kWh with VAT on top.
pub fn eur_per_kwh(eur_per_mwh: f32, vat_pct: f32) -> f32 {
    eur_per_mwh / 1000.0 * (1.0 + vat_pct / 100.0)
}

/// Bucket €/MWh samples into the 96 local quarter-hours of `day` as €/kWh
/// with VAT. Missing slots are NaN; several samples in one slot (the repeated
/// hour of a fall-back day) average.
pub fn slots_for_day<Z: TimeZone>(
    samples: &[(DateTime<Utc>, f32)],
    day: NaiveDate,
    tz: &Z,
    vat_pct: f32,
) -> Vec<f32> {
    let mut sum = [0.0f32; SLOTS_PER_DAY];
    let mut cnt = [0u32; SLOTS_PER_DAY];
    for (t, mwh) in samples {
        let local = t.with_timezone(tz);
        if local.date_naive() != day {
            continue;
        }
        let slot = (local.hour() * 4 + local.minute() / 15) as usize;
        sum[slot] += eur_per_kwh(*mwh, vat_pct);
        cnt[slot] += 1;
    }
    (0..SLOTS_PER_DAY)
        .map(|i| {
            if cnt[i] == 0 {
                f32::NAN
            } else {
                sum[i] / cnt[i] as f32
            }
        })
        .collect()
}

/// Mean €/kWh (VAT included) over every sample whose local date is one of
/// `days`; NaN when none is.
pub fn mean_over_days<Z: TimeZone>(
    samples: &[(DateTime<Utc>, f32)],
    days: &[NaiveDate],
    tz: &Z,
    vat_pct: f32,
) -> f32 {
    let vals: Vec<f32> = samples
        .iter()
        .filter(|(t, _)| days.contains(&t.with_timezone(tz).date_naive()))
        .map(|(_, mwh)| eur_per_kwh(*mwh, vat_pct))
        .collect();
    if vals.is_empty() {
        f32::NAN
    } else {
        vals.iter().sum::<f32>() / vals.len() as f32
    }
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

/// ENTSO-E Publication_MarketDocument -> (ISO timestamp, price per MWh) for
/// every position of every Period.
///
/// `curveType` A03 ("variable sized block") leaves out any position whose price
/// equals the previous one, so a period is read as a sparse list and then filled
/// in: every position from 1 to the count the period's `timeInterval` implies
/// carries the last price seen, and the positions before the first point take
/// the first price.
pub fn parse_entsoe(xml: &str) -> Result<Vec<(String, f32)>> {
    use quick_xml::events::Event as X;
    use quick_xml::Reader;
    let mut reader = Reader::from_str(xml);
    let mut out = Vec::new();
    let mut path: Vec<String> = Vec::new();
    let mut text = String::new();
    // the Period being read
    let mut start: Option<DateTime<Utc>> = None;
    let mut end: Option<DateTime<Utc>> = None;
    let mut resolution_min: i64 = 60;
    let mut position: i64 = 0;
    let mut points: Vec<(i64, f32)> = Vec::new();
    loop {
        match reader.read_event().context("xml")? {
            X::Start(e) => {
                let name = e.name().as_ref().to_string();
                if name == "Period" {
                    (start, end, resolution_min) = (None, None, 60);
                    points.clear();
                }
                path.push(name);
                text.clear();
            }
            X::Text(t) => text = t.xml10_content().into_owned(),
            X::End(_) => {
                let tag = path.pop().unwrap_or_default();
                let parent = path.last().map(String::as_str).unwrap_or("");
                // the document repeats the interval as `period.timeInterval`;
                // only the one inside a Period dates its points
                let in_period = path.iter().any(|p| p == "Period");
                match (parent, tag.as_str()) {
                    ("timeInterval", "start") if in_period => start = parse_utc(&text),
                    ("timeInterval", "end") if in_period => end = parse_utc(&text),
                    ("Period", "resolution") => {
                        resolution_min = match text.as_str() {
                            "PT15M" => 15,
                            "PT30M" => 30,
                            _ => 60,
                        }
                    }
                    ("Point", "position") => position = text.trim().parse().unwrap_or(0),
                    ("Point", "price.amount") => {
                        if let Ok(p) = text.trim().parse::<f32>() {
                            points.push((position, p));
                        }
                    }
                    (_, "Period") => out.extend(fill_period(start, end, resolution_min, &points)),
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

/// One timestamped price per position of a period, carrying prices forward over
/// the positions the document leaves out.
fn fill_period(
    start: Option<DateTime<Utc>>,
    end: Option<DateTime<Utc>>,
    resolution_min: i64,
    points: &[(i64, f32)],
) -> Vec<(String, f32)> {
    let (Some(s), false) = (start, points.is_empty()) else {
        return Vec::new();
    };
    let last_pos = points.iter().map(|(p, _)| *p).max().unwrap_or(0);
    let expected = end
        .map(|e| (e - s).num_minutes() / resolution_min.max(1))
        .unwrap_or(0);
    let n = expected.max(last_pos);
    let mut carried = points
        .iter()
        .min_by_key(|(p, _)| *p)
        .map(|(_, v)| *v)
        .expect("points is not empty");
    let mut out = Vec::with_capacity(n.max(0) as usize);
    for pos in 1..=n {
        if let Some((_, v)) = points.iter().find(|(p, _)| *p == pos) {
            carried = *v;
        }
        let ts = s + chrono::Duration::minutes((pos - 1) * resolution_min);
        out.push((ts.to_rfc3339(), carried));
    }
    out
}

/// Bucket timestamped prices into the 24 local hours of `day`, converting to ct/kWh.
/// `per_kwh` = input is €/kWh (EnergyZero) else €/MWh (ENTSO-E). Missing hours are NaN;
/// sub-hourly points average into their hour.
pub fn hourly_ct_for_day<Z: TimeZone>(
    points: &[(String, f32)],
    day: NaiveDate,
    tz: &Z,
    per_kwh: bool,
) -> Vec<f32> {
    let mut sum = [0.0f32; 24];
    let mut cnt = [0u32; 24];
    for (ts, price) in points {
        let Some(t) = parse_utc(ts) else { continue };
        let local = t.with_timezone(tz);
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

fn day_bounds_utc<Z: TimeZone>(day: NaiveDate, tz: &Z) -> (DateTime<Utc>, DateTime<Utc>) {
    let midnight = day.and_hms_opt(0, 0, 0).unwrap();
    let start = tz
        .from_local_datetime(&midnight)
        .single()
        .unwrap_or_else(|| tz.from_utc_datetime(&midnight));
    // a local day, so the DST days stay 23 or 25 hours long
    let end = start.clone() + chrono::Duration::days(1);
    (start.with_timezone(&Utc), end.with_timezone(&Utc))
}

/// A reqwest error prints the URL it came from, and the ENTSO-E URL carries the
/// security token as a query parameter. Drop the URL before the error reaches a
/// log line; the context added by the caller says which request failed.
fn redact(e: reqwest::Error) -> anyhow::Error {
    anyhow::Error::new(e.without_url())
}

async fn fetch_day(cfg: &PriceConfig, day: NaiveDate) -> Result<Vec<f32>> {
    let (from, till) = day_bounds_utc(day, &cfg.tz);
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
                .map_err(redact)
                .context("energyzero request")?
                .error_for_status()
                .map_err(redact)
                .context("energyzero status")?
                .text()
                .await
                .map_err(redact)
                .context("energyzero body")?;
            Ok(hourly_ct_for_day(
                &parse_energyzero(&body)?,
                day,
                &cfg.tz,
                true,
            ))
        }
        PriceSource::Entsoe { token, zone } => {
            // the token goes in through `query`, never into a formatted URL we
            // could accidentally log
            let period_start = from.format("%Y%m%d%H%M").to_string();
            let period_end = till.format("%Y%m%d%H%M").to_string();
            let body = client()
                .get(ENTSOE_URL)
                .query(&[
                    ("securityToken", token.as_str()),
                    ("documentType", "A44"),
                    ("in_Domain", zone.as_str()),
                    ("out_Domain", zone.as_str()),
                    ("periodStart", period_start.as_str()),
                    ("periodEnd", period_end.as_str()),
                ])
                .send()
                .await
                .map_err(redact)
                .context("entsoe request")?
                .error_for_status()
                .map_err(redact)
                .context("entsoe status")?
                .text()
                .await
                .map_err(redact)
                .context("entsoe body")?;
            Ok(hourly_ct_for_day(
                &parse_entsoe(&body)?,
                day,
                &cfg.tz,
                false,
            ))
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
    const ENTSOE_A03: &str = include_str!("../tests/fixtures/entsoe-a03.xml");

    #[test]
    fn energyzero_to_local_hours() {
        let pts = parse_energyzero(EZ).unwrap();
        assert_eq!(pts.len(), 24);
        let day = NaiveDate::from_ymd_opt(2026, 9, 6).unwrap();
        let ct = hourly_ct_for_day(&pts, day, &chrono_tz::Europe::Amsterdam, true);
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
        let ct = hourly_ct_for_day(&pts, day, &chrono_tz::Europe::Amsterdam, false);
        assert!((ct[0] - 8.55).abs() < 0.01, "€/MWh to ct/kWh");
        assert!(parse_entsoe("<x/>").is_err());
    }

    #[tokio::test]
    async fn request_errors_never_carry_the_token() {
        // an unroutable port: the request fails before anything is sent
        let err = client()
            .get("http://127.0.0.1:1/api")
            .query(&[("securityToken", "SECRETTOKEN"), ("documentType", "A44")])
            .send()
            .await
            .map_err(redact)
            .context("entsoe request")
            .expect_err("connection refused");
        let text = format!("{err:#} {err:?}");
        assert!(!text.contains("SECRETTOKEN"), "leaked the token: {text}");
        assert!(!text.contains("securityToken"), "leaked the query: {text}");
        assert!(text.contains("entsoe request"), "kept the context: {text}");
    }

    #[test]
    fn entsoe_a03_carries_the_missing_positions_forward() {
        // curveType A03 only publishes a point when the price changes: three
        // points stand for a whole day
        let pts = parse_entsoe(ENTSOE_A03).unwrap();
        assert_eq!(pts.len(), 24, "one point per hour of the interval");
        let v: Vec<f32> = pts.iter().map(|(_, p)| *p).collect();
        assert_eq!(&v[0..2], &[85.5, 85.5], "position 2 repeats position 1");
        assert_eq!(&v[2..9], &[76.4; 7], "3..9 repeat position 3");
        assert_eq!(&v[9..24], &[-4.25; 15], "10 onwards repeat position 10");
        // hourly stamps, an hour apart, starting at the period start
        assert_eq!(pts[0].0, "2026-09-05T22:00:00+00:00");
        assert_eq!(pts[23].0, "2026-09-06T21:00:00+00:00");
        let day = NaiveDate::from_ymd_opt(2026, 9, 6).unwrap();
        let ct = hourly_ct_for_day(&pts, day, &chrono_tz::Europe::Amsterdam, false);
        assert_eq!(ct.iter().filter(|v| v.is_finite()).count(), 24);
        assert!((ct[0] - 8.55).abs() < 0.01);
        assert!(ct[23] < 0.0, "the cheap evening stays negative");
    }

    #[test]
    fn the_local_zone_buckets_into_the_hour_the_marker_shows() {
        use chrono::Timelike;
        // `runloop` marks the current hour with `chrono::Local`; bucketing has to
        // agree with it, whatever the Pi's system zone is
        let now = Local::now();
        let ct = hourly_ct_for_day(
            &[(now.with_timezone(&Utc).to_rfc3339(), 0.1)],
            now.date_naive(),
            &Local,
            true,
        );
        let h = now.hour() as usize;
        assert!((ct[h] - 10.0).abs() < 0.01, "hour {h} of {ct:?}");
        assert_eq!(ct.iter().filter(|v| v.is_finite()).count(), 1);
    }

    #[test]
    fn missing_hours_are_nan_and_zones_map() {
        let day = NaiveDate::from_ymd_opt(2026, 9, 6).unwrap();
        let ct = hourly_ct_for_day(
            &[("2026-09-06T10:00:00Z".into(), 0.1)],
            day,
            &chrono_tz::Europe::Amsterdam,
            true,
        );
        assert!(ct[12].is_finite() && ct[11].is_nan());
        assert_eq!(entsoe_zone_for("nl"), Some("10YNL----------L"));
        assert_eq!(entsoe_zone_for("XX"), None);
    }

    const EC: &str = include_str!("../tests/fixtures/energy-charts-price.json");

    #[test]
    fn energy_charts_fixture_parses_three_days_of_quarter_hours() {
        let pts = parse_energy_charts(EC).unwrap();
        assert_eq!(pts.len(), 288, "3 days x 96 slots");
        // 2026-09-09T00:00+02:00 is 22:00Z the day before
        assert_eq!(
            pts[0].0,
            Utc.with_ymd_and_hms(2026, 9, 8, 22, 0, 0).unwrap()
        );
        assert_eq!(pts[0].1, 150.31);
        assert_eq!(pts[287].1, 192.05);
        assert!(parse_energy_charts("{}").is_err(), "no data array");
        assert!(parse_energy_charts(r#"{"data":[]}"#).is_err(), "empty data");
        // a null price is skipped, not zero
        let one = parse_energy_charts(
            r#"{"data":[{"timestamp":"2026-09-11T10:00:00+02:00","values":{"day_ahead_price":null}},
                        {"timestamp":"2026-09-11T10:15:00+02:00","values":{"day_ahead_price":12.5}}]}"#,
        )
        .unwrap();
        assert_eq!(one.len(), 1);
        assert_eq!(one[0].1, 12.5);
    }

    #[test]
    fn energy_charts_slots_are_local_quarter_hours_in_euro_with_vat() {
        let pts = parse_energy_charts(EC).unwrap();
        let day = NaiveDate::from_ymd_opt(2026, 9, 11).unwrap();
        let eur = slots_for_day(&pts, day, &chrono_tz::Europe::Amsterdam, 21.0);
        assert_eq!(eur.len(), SLOTS_PER_DAY);
        assert_eq!(eur.iter().filter(|v| v.is_finite()).count(), 96);
        // fixture: 2026-09-11 00:00 local is 183.9 €/MWh, 14:00 local is 154.42
        assert!((eur[0] - 0.1839 * 1.21).abs() < 1e-5, "{}", eur[0]);
        assert!((eur[56] - 0.15442 * 1.21).abs() < 1e-5, "{}", eur[56]);
        // the two other days are not in today's slots
        let other = NaiveDate::from_ymd_opt(2026, 9, 12).unwrap();
        assert!(
            slots_for_day(&pts, other, &chrono_tz::Europe::Amsterdam, 21.0)
                .iter()
                .all(|v| v.is_nan())
        );
        // the mean over the three requested days, VAT included
        let days = [
            NaiveDate::from_ymd_opt(2026, 9, 9).unwrap(),
            NaiveDate::from_ymd_opt(2026, 9, 10).unwrap(),
            day,
        ];
        let avg = mean_over_days(&pts, &days, &chrono_tz::Europe::Amsterdam, 21.0);
        assert!((avg - 0.217646).abs() < 1e-5, "{avg}");
        // only today: a different mean; no matching day: NaN
        let today_only = mean_over_days(&pts, &[day], &chrono_tz::Europe::Amsterdam, 0.0);
        assert!(today_only.is_finite() && (today_only - avg).abs() > 1e-4);
        assert!(mean_over_days(&pts, &[other], &chrono_tz::Europe::Amsterdam, 0.0).is_nan());
        assert!((eur_per_kwh(100.0, 0.0) - 0.1).abs() < 1e-7);
        assert!((eur_per_kwh(100.0, 21.0) - 0.121).abs() < 1e-7);
    }

    #[test]
    fn quarter_hour_slots_survive_dst_days() {
        let ams = chrono_tz::Europe::Amsterdam;
        // spring forward, 2026-03-29: 02:00 local does not exist
        let day = NaiveDate::from_ymd_opt(2026, 3, 29).unwrap();
        let pts = vec![
            (Utc.with_ymd_and_hms(2026, 3, 29, 0, 0, 0).unwrap(), 100.0), // 01:00 CET
            (Utc.with_ymd_and_hms(2026, 3, 29, 1, 0, 0).unwrap(), 200.0), // 03:00 CEST
        ];
        let eur = slots_for_day(&pts, day, &ams, 0.0);
        assert!((eur[4] - 0.1).abs() < 1e-6, "01:00 is slot 4");
        assert!(eur[8..12].iter().all(|v| v.is_nan()), "02:xx never happens");
        assert!((eur[12] - 0.2).abs() < 1e-6, "03:00 is slot 12");
        // fall back, 2026-10-25: 02:30 local happens twice and the two average
        let day = NaiveDate::from_ymd_opt(2026, 10, 25).unwrap();
        let pts = vec![
            (Utc.with_ymd_and_hms(2026, 10, 25, 0, 30, 0).unwrap(), 100.0), // 02:30 CEST
            (Utc.with_ymd_and_hms(2026, 10, 25, 1, 30, 0).unwrap(), 200.0), // 02:30 CET
        ];
        let eur = slots_for_day(&pts, day, &ams, 0.0);
        assert!(
            (eur[10] - 0.15).abs() < 1e-6,
            "both 02:30s share slot 10: {}",
            eur[10]
        );
        assert_eq!(eur.iter().filter(|v| v.is_finite()).count(), 1);
    }

    #[test]
    fn the_local_zone_buckets_into_the_slot_the_gauge_reads() {
        use chrono::Timelike;
        // `runloop` reads the current slot from `chrono::Local`; bucketing has
        // to agree with it, whatever the Pi's system zone is
        let now = Local::now();
        let eur = slots_for_day(
            &[(now.with_timezone(&Utc), 100.0)],
            now.date_naive(),
            &Local,
            0.0,
        );
        let slot = (now.hour() * 4 + now.minute() / 15) as usize;
        assert!((eur[slot] - 0.1).abs() < 1e-6, "slot {slot} of {eur:?}");
        assert_eq!(eur.iter().filter(|v| v.is_finite()).count(), 1);
    }
}
