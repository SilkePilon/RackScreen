//! Day-ahead electricity prices from Energy-Charts (Fraunhofer ISE), no token.

use std::time::Duration;

use anyhow::{Context, Result};
use chrono::{DateTime, Local, NaiveDate, TimeZone, Timelike, Utc};
use rackscreen_core::event::{Event, LinkTarget};
use serde_json::Value;

use crate::http::client;
use crate::SourceCtx;

#[derive(Clone, Debug)]
pub struct PriceConfig {
    /// Energy-Charts bidding zone, for example `NL` or `DE-LU`.
    pub zone: String,
    /// VAT added to the raw exchange price.
    pub vat_pct: f32,
    pub poll_secs: u64,
    /// The system local zone, the same clock `runloop` uses for the current
    /// slot; set the Pi's zone with `timedatectl`.
    pub tz: Local,
}

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

/// One request for the days `first..=last`, inclusive. Energy-Charts reads
/// both bounds in the bidding zone's time zone, not the Pi's, so callers ask
/// for a day more than they need.
async fn fetch_days(
    cfg: &PriceConfig,
    first: NaiveDate,
    last: NaiveDate,
) -> Result<Vec<(DateTime<Utc>, f32)>> {
    let body = client()
        .get(ENERGY_CHARTS_URL)
        .query(&[
            ("bzn", cfg.zone.as_str()),
            ("start", &first.to_string()),
            ("end", &last.to_string()),
        ])
        .send()
        .await
        .context("energy-charts request")?
        .error_for_status()
        .context("energy-charts status")?
        .text()
        .await
        .context("energy-charts body")?;
    parse_energy_charts(&body)
}

pub async fn run_prices(cfg: PriceConfig, ctx: SourceCtx) {
    let mut failures = 0u32;
    loop {
        if ctx.shutdown.is_cancelled() {
            return;
        }
        let today = Utc::now().with_timezone(&cfg.tz).date_naive();
        let first = today - chrono::Duration::days(2);
        let days = [first, first + chrono::Duration::days(1), today];
        // ask through tomorrow: Energy-Charts reads start/end in the bidding
        // zone's own time zone, so a Pi west of the zone (Amsterdam polling FI)
        // would otherwise never receive its own late evening. The mean stays on
        // the three `days` and `slots_for_day(today)` drops the rest.
        match fetch_days(&cfg, first, today + chrono::Duration::days(1)).await {
            Ok(samples) => {
                failures = 0;
                let eur = slots_for_day(&samples, today, &cfg.tz, cfg.vat_pct);
                let avg = mean_over_days(&samples, &days, &cfg.tz, cfg.vat_pct);
                tracing::info!(
                    "prices: {} of {SLOTS_PER_DAY} quarter-hours for {today}, three-day mean {avg:.3} €/kWh",
                    eur.iter().filter(|v| v.is_finite()).count()
                );
                ctx.emit(Event::Link {
                    target: LinkTarget::Prices,
                    up: true,
                });
                ctx.emit(Event::Prices {
                    date: today.to_string(),
                    eur_per_kwh: eur,
                    avg_eur_per_kwh: avg,
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
        // the gauge follows the wall clock on its own; re-poll on the interval,
        // and at the latest just after local midnight for the new day
        let now_local = Utc::now().with_timezone(&cfg.tz);
        let to_midnight = 86_400
            - (now_local.hour() * 3600 + now_local.minute() * 60 + now_local.second()) as u64
            + 5;
        let normal = cfg.poll_secs.max(900);
        let wait = if failures > 0 {
            120
        } else {
            normal.min(to_midnight)
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
    fn slots_follow_the_pi_zone_not_the_bidding_zone() {
        // the fixture is NL (+02:00): the day a Pi in another zone calls
        // 2026-09-11 reaches into the neighbouring NL days, which is why the
        // request runs through tomorrow.
        let pts = parse_energy_charts(EC).unwrap();
        let day = NaiveDate::from_ymd_opt(2026, 9, 11).unwrap();
        // Helsinki is an hour ahead: its 23:00-23:45 is 22:00-22:45 in NL
        let eet = slots_for_day(&pts, day, &chrono_tz::Europe::Helsinki, 21.0);
        assert!(
            eet[92..96].iter().all(|v| v.is_finite()),
            "the Helsinki evening: {:?}",
            &eet[92..96]
        );
        // the Azores are two hours behind: their 00:00-00:45 is 02:00-02:45 in NL
        let azo = slots_for_day(&pts, day, &chrono_tz::Atlantic::Azores, 21.0);
        assert!(
            azo[0..4].iter().all(|v| v.is_finite()),
            "the Azorean small hours: {:?}",
            &azo[0..4]
        );
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
