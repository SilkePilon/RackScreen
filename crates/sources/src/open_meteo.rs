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
        "https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&current=temperature_2m,weather_code,wind_speed_10m,wind_direction_10m,wind_gusts_10m,is_day&timezone=UTC",
        cfg.lat, cfg.lon
    )
}

pub fn air_quality_url(cfg: &WeatherConfig) -> String {
    format!(
        "https://air-quality-api.open-meteo.com/v1/air-quality?latitude={}&longitude={}&current=european_aqi",
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
        match fetch(&forecast_url(&cfg))
            .await
            .and_then(|b| parse_forecast(&b))
        {
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
                // air quality rides along; a miss here is a warning, not a link edge
                match fetch(&air_quality_url(&cfg))
                    .await
                    .and_then(|b| parse_air_quality(&b))
                {
                    Ok(eaqi) => ctx.emit(Event::AirQuality { eaqi }),
                    // no `weather:` prefix: the link is deliberately still up,
                    // and the Status dot keys off that needle.
                    Err(e) => tracing::warn!("air quality: {e:#}"),
                }
            }
            Err(e) => {
                failures += 1;
                tracing::warn!("weather: {e:#}");
                if failures >= 2 {
                    ctx.emit(Event::Link {
                        target: LinkTarget::Weather,
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
        assert_eq!(
            forecast_url(&cfg),
            "https://api.open-meteo.com/v1/forecast?latitude=52.37&longitude=4.89&current=temperature_2m,weather_code,wind_speed_10m,wind_direction_10m,wind_gusts_10m,is_day&timezone=UTC"
        );
        assert_eq!(
            air_quality_url(&cfg),
            "https://air-quality-api.open-meteo.com/v1/air-quality?latitude=52.37&longitude=4.89&current=european_aqi"
        );
    }
}
