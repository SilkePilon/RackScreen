//! Open-Meteo poller: current conditions and the European air quality index.
//! No key; the free tier allows 10 000 calls a day.

use std::time::Duration;

use anyhow::{Context, Result};
use rackscreen_core::event::{Event, LinkTarget};
use serde_json::Value;

use crate::electricity::{backoff_secs, HttpStatus};
use crate::http::client;
use crate::SourceCtx;

#[derive(Clone, Debug)]
pub struct WeatherConfig {
    pub lat: f64,
    pub lon: f64,
    pub poll_secs: u64,
}

pub fn forecast_url(cfg: &WeatherConfig) -> String {
    format!(
        "https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&current=temperature_2m,weather_code,wind_speed_10m,wind_direction_10m,wind_gusts_10m,is_day&hourly=temperature_2m,precipitation&daily=temperature_2m_min,temperature_2m_max&forecast_hours=12&forecast_days=1&timezone=UTC",
        cfg.lat, cfg.lon
    )
}

pub fn air_quality_url(cfg: &WeatherConfig) -> String {
    format!(
        "https://air-quality-api.open-meteo.com/v1/air-quality?latitude={}&longitude={}&current=european_aqi&hourly=european_aqi&past_days=1&forecast_days=1&timezone=UTC",
        cfg.lat, cfg.lon
    )
}

fn num(cur: &Value, key: &str) -> Result<f64> {
    cur.get(key)
        .and_then(Value::as_f64)
        .with_context(|| format!("current.{key} missing"))
}

pub fn parse_forecast(json: &str) -> Result<Event> {
    let v: Value = serde_json::from_str(json).context("forecast json")?;
    let cur = v.get("current").context("current missing")?;
    Ok(Event::Weather {
        temp_c: num(cur, "temperature_2m")? as f32,
        code: num(cur, "weather_code")? as u16,
        is_day: num(cur, "is_day").unwrap_or(1.0) > 0.0,
        wind_kmh: num(cur, "wind_speed_10m").unwrap_or(0.0) as f32,
        gust_kmh: num(cur, "wind_gusts_10m").unwrap_or(0.0) as f32,
        wind_from_deg: num(cur, "wind_direction_10m").unwrap_or(0.0) as f32,
        at: cur
            .get("time")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
    })
}

pub fn parse_air_quality(json: &str) -> Result<f32> {
    let v: Value = serde_json::from_str(json).context("air quality json")?;
    let cur = v.get("current").context("current missing")?;
    Ok(num(cur, "european_aqi")? as f32)
}

fn f32s(v: &Value, path: &[&str]) -> Result<Vec<Option<f32>>> {
    let mut cur = v;
    for p in path {
        cur = cur
            .get(*p)
            .with_context(|| format!("{} missing", path.join(".")))?;
    }
    Ok(cur
        .as_array()
        .with_context(|| format!("{} is not an array", path.join(".")))?
        .iter()
        .map(|x| x.as_f64().map(|f| f as f32))
        .collect())
}

/// The hourly and daily blocks of the same forecast response.
pub fn parse_forecast_hours(json: &str) -> Result<Event> {
    let v: Value = serde_json::from_str(json).context("forecast json")?;
    let temps = f32s(&v, &["hourly", "temperature_2m"])?;
    let precip = f32s(&v, &["hourly", "precipitation"])?;
    let min = f32s(&v, &["daily", "temperature_2m_min"])?;
    let max = f32s(&v, &["daily", "temperature_2m_max"])?;
    Ok(Event::Forecast {
        hourly_temp_c: temps.into_iter().map(|t| t.unwrap_or(f32::NAN)).collect(),
        hourly_precip_mm: precip.into_iter().map(|p| p.unwrap_or(0.0)).collect(),
        day_min_c: min.first().copied().flatten().context("daily min")?,
        day_max_c: max.first().copied().flatten().context("daily max")?,
    })
}

/// Unix seconds of an Open-Meteo `YYYY-MM-DDTHH:MM` stamp (UTC).
fn stamp_unix(s: &str) -> Option<i64> {
    chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M")
        .ok()
        .map(|d| d.and_utc().timestamp())
}

/// The 24 hourly values ending at the hour of `now_unix`, oldest first, nulls skipped.
pub fn parse_air_quality_history(json: &str, now_unix: i64) -> Result<Vec<f32>> {
    let v: Value = serde_json::from_str(json).context("air quality json")?;
    let times = v
        .get("hourly")
        .and_then(|h| h.get("time"))
        .and_then(Value::as_array)
        .context("hourly.time missing")?;
    let values = f32s(&v, &["hourly", "european_aqi"])?;
    let hour = now_unix.div_euclid(3600) * 3600;
    let end = times
        .iter()
        .rposition(|t| t.as_str().and_then(stamp_unix).is_some_and(|u| u <= hour))
        .context("no hourly value up to now")?;
    let start = (end + 1).saturating_sub(24);
    Ok(values[start..=end].iter().flatten().copied().collect())
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

pub async fn run_weather(cfg: WeatherConfig, ctx: SourceCtx) {
    let mut failures = 0u32;
    loop {
        if ctx.shutdown.is_cancelled() {
            return;
        }
        match fetch(&forecast_url(&cfg)).await {
            Ok(body) => match parse_forecast(&body) {
                Ok(ev) => {
                    failures = 0;
                    if let Event::Weather { temp_c, code, .. } = &ev {
                        tracing::info!("weather: poll ok ({temp_c:.1} °C, code {code})");
                    }
                    ctx.emit(Event::Link {
                        target: LinkTarget::Weather,
                        up: true,
                    });
                    ctx.emit(ev);
                    match parse_forecast_hours(&body) {
                        Ok(fc) => ctx.emit(fc),
                        Err(e) => tracing::warn!("forecast hours: {e:#}"),
                    }
                    // air quality rides along; a miss here is a warning, not a link edge
                    // (no `weather:` prefix: the Status dot keys off that needle)
                    match fetch(&air_quality_url(&cfg)).await {
                        Ok(aq) => {
                            match parse_air_quality(&aq) {
                                Ok(eaqi) => ctx.emit(Event::AirQuality { eaqi }),
                                Err(e) => tracing::warn!("air quality: {e:#}"),
                            }
                            let now = chrono::Utc::now().timestamp();
                            match parse_air_quality_history(&aq, now) {
                                Ok(hourly) => ctx.emit(Event::AirQualityHistory { hourly }),
                                Err(e) => tracing::warn!("air quality history: {e:#}"),
                            }
                        }
                        Err(e) => tracing::warn!("air quality: {e:#}"),
                    }
                }
                Err(e) => {
                    failures += 1;
                    tracing::warn!("weather: {e:#}");
                }
            },
            Err(e) => {
                failures += 1;
                tracing::warn!("weather: {e:#}");
            }
        }
        if failures >= 2 {
            ctx.emit(Event::Link {
                target: LinkTarget::Weather,
                up: false,
            });
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

    const FC: &str = include_str!("../tests/fixtures/open-meteo-forecast.json");
    const AQ: &str = include_str!("../tests/fixtures/open-meteo-air-quality.json");

    #[test]
    fn parses_current_conditions() {
        let ev = parse_forecast(FC).unwrap();
        assert_eq!(
            ev,
            Event::Weather {
                temp_c: 21.2,
                code: 3,
                is_day: false,
                wind_kmh: 19.1,
                gust_kmh: 38.9,
                wind_from_deg: 232.0,
                at: "2026-09-07T19:45".into(),
            }
        );
        assert!(parse_forecast("{}").is_err());
        assert!(
            parse_forecast("{\"current\": {\"temperature_2m\": 1}}").is_err(),
            "code missing"
        );
    }

    #[test]
    fn parses_air_quality() {
        assert_eq!(parse_air_quality(AQ).unwrap(), 40.0);
        assert!(parse_air_quality("{\"current\": {}}").is_err());
    }

    #[test]
    fn urls_carry_the_location() {
        let cfg = WeatherConfig {
            lat: 52.37,
            lon: 4.89,
            poll_secs: 600,
        };
        assert!(forecast_url(&cfg).starts_with("https://api.open-meteo.com/v1/forecast?latitude=52.37&longitude=4.89&current=temperature_2m,weather_code,wind_speed_10m,wind_direction_10m,wind_gusts_10m,is_day"));
        assert!(air_quality_url(&cfg).starts_with("https://air-quality-api.open-meteo.com/v1/air-quality?latitude=52.37&longitude=4.89&current=european_aqi"));
    }
    const FCH: &str = include_str!("../tests/fixtures/open-meteo-forecast-hourly.json");
    const AQH: &str = include_str!("../tests/fixtures/open-meteo-air-quality-hourly.json");

    #[test]
    fn parses_hourly_forecast_and_daily_range() {
        let ev = parse_forecast_hours(FCH).unwrap();
        assert_eq!(
            ev,
            Event::Forecast {
                hourly_temp_c: vec![
                    13.4, 13.6, 13.3, 12.3, 12.4, 12.0, 11.8, 11.6, 11.2, 11.5, 11.3, 10.8
                ],
                hourly_precip_mm: vec![0.0, 0.0, 0.0, 1.1, 0.1, 0.1, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                day_min_c: 10.3,
                day_max_c: 13.6,
            }
        );
        assert!(
            parse_forecast_hours(FC).is_err(),
            "the old fixture has no hourly block"
        );
    }

    #[test]
    fn air_history_ends_at_the_current_hour_and_skips_nulls() {
        // fixture: 2026-11-01T00:00 .. 2026-11-02T23:00 UTC, index 20 is null
        let now = 1_793_620_800; // 2026-11-02T12:00:00Z
        let h = parse_air_quality_history(AQH, now).unwrap();
        assert_eq!(
            h.len(),
            23,
            "24 hours ending at 12:00 minus the null at 20:00 yesterday"
        );
        assert_eq!(h[0], 55.0, "13:00 yesterday");
        assert_eq!(*h.last().unwrap(), 26.0, "12:00 today");
        assert_eq!(
            parse_air_quality_history(AQH, 1_793_577_600 - 7200)
                .unwrap()
                .len(),
            22,
            "22:00 yesterday: the 23 values up to it minus the null"
        );
    }

    #[test]
    fn urls_ask_for_the_hourly_blocks() {
        let cfg = WeatherConfig {
            lat: 52.37,
            lon: 4.89,
            poll_secs: 600,
        };
        assert!(forecast_url(&cfg).contains("&hourly=temperature_2m,precipitation&daily=temperature_2m_min,temperature_2m_max&forecast_hours=12&forecast_days=1&timezone=UTC"));
        assert!(air_quality_url(&cfg).ends_with(
            "&current=european_aqi&hourly=european_aqi&past_days=1&forecast_days=1&timezone=UTC"
        ));
    }
}
