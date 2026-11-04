//! Home Assistant entity states over the REST API (`/api/states/<id>`) with
//! a long-lived access token.

use std::time::Duration;

use rackscreen_core::event::{Event, HaEntity, HaStatus};
use serde_json::Value;

use crate::http::{client, read_capped};
use crate::SourceCtx;

/// One entity a screen shows.
#[derive(Clone, Debug, PartialEq)]
pub struct HaQuery {
    /// Index into the config's `homeassistant.entities`.
    pub index: u16,
    /// `sensor.living_room_temperature`.
    pub entity: String,
    /// Read this attribute instead of the state (`current_temperature`).
    pub attribute: Option<String>,
    /// Use this unit instead of the entity's own.
    pub unit: Option<String>,
}

#[derive(Clone, Debug)]
pub struct HaConfig {
    /// `http://homeassistant.local:8123`, no trailing slash needed.
    pub url: String,
    pub token: String,
    pub poll_secs: u64,
    pub entities: Vec<HaQuery>,
}

/// An attribute value as state text: strings as they are, numbers printed.
fn as_text(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(if *b { "on" } else { "off" }.into()),
        _ => None,
    }
}

fn unix(ts: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(ts)
        .ok()
        .map(|t| t.timestamp())
}

/// The entity from a `/api/states/<id>` body.
pub fn parse_state(body: &str, q: &HaQuery) -> Option<HaEntity> {
    let v: Value = serde_json::from_str(body).ok()?;
    let id = v.get("entity_id")?.as_str()?;
    let attrs = v.get("attributes").cloned().unwrap_or(Value::Null);
    let attr = |k: &str| attrs.get(k).and_then(Value::as_str).map(str::to_string);
    let state = match &q.attribute {
        Some(a) => attrs
            .get(a)
            .and_then(as_text)
            .unwrap_or_else(|| "unknown".into()),
        None => v.get("state").and_then(as_text)?,
    };
    let unit = match (&q.unit, &q.attribute) {
        (Some(u), _) => Some(u.clone()),
        // the entity's unit belongs to its state, not to an attribute
        (None, Some(_)) => None,
        (None, None) => attr("unit_of_measurement"),
    };
    Some(HaEntity {
        state,
        name: attr("friendly_name").unwrap_or_else(|| id.to_string()),
        unit: unit.filter(|u| !u.trim().is_empty()),
        device_class: attr("device_class"),
        domain: id.split('.').next().unwrap_or("").to_string(),
        last_changed: v.get("last_changed").and_then(Value::as_str).and_then(unix),
    })
}

/// The states URL for an entity; the id is checked by the config.
pub fn state_url(base: &str, entity: &str) -> String {
    format!("{}/api/states/{entity}", base.trim().trim_end_matches('/'))
}

async fn poll(cfg: &HaConfig, q: &HaQuery) -> (HaStatus, Option<HaEntity>) {
    let resp = client()
        .get(state_url(&cfg.url, &q.entity))
        .bearer_auth(cfg.token.trim())
        .send()
        .await;
    let resp = match resp {
        Ok(r) => r,
        Err(e) => {
            tracing::debug!("homeassistant: {e}");
            return (HaStatus::Offline, None);
        }
    };
    match resp.status().as_u16() {
        200 => {}
        401 | 403 => return (HaStatus::Unauthorized, None),
        404 => return (HaStatus::NotFound, None),
        code => {
            tracing::debug!("homeassistant: {} answered {code}", q.entity);
            return (HaStatus::Offline, None);
        }
    }
    let body = match read_capped(resp, 256 * 1024, "entity state").await {
        Ok(b) => b,
        Err(e) => {
            tracing::debug!("homeassistant: {e:#}");
            return (HaStatus::Offline, None);
        }
    };
    match parse_state(&String::from_utf8_lossy(&body), q) {
        Some(e) => (HaStatus::Ok, Some(e)),
        None => (HaStatus::Offline, None),
    }
}

pub async fn run_homeassistant(cfg: HaConfig, ctx: SourceCtx) {
    let mut last: Vec<Option<HaStatus>> = vec![None; cfg.entities.len()];
    loop {
        for (i, q) in cfg.entities.iter().enumerate() {
            if ctx.shutdown.is_cancelled() {
                return;
            }
            let (status, entity) = poll(&cfg, q).await;
            if last[i] != Some(status) {
                match status {
                    HaStatus::Ok => tracing::info!("homeassistant: {} ok", q.entity),
                    HaStatus::Offline => {
                        tracing::warn!("homeassistant: {} unreachable at {}", q.entity, cfg.url)
                    }
                    HaStatus::Unauthorized => tracing::warn!(
                        "homeassistant: token refused; make a long-lived token in your profile"
                    ),
                    HaStatus::NotFound => {
                        tracing::warn!("homeassistant: no entity {}", q.entity)
                    }
                }
                last[i] = Some(status);
            }
            ctx.emit(Event::HomeAssistant {
                index: q.index,
                status,
                entity,
            });
        }
        tokio::select! {
            _ = ctx.shutdown.cancelled() => return,
            _ = tokio::time::sleep(Duration::from_secs(cfg.poll_secs.max(5))) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STATE: &str = r#"{
        "entity_id": "sensor.living_room_temperature",
        "state": "21.4",
        "attributes": {
            "unit_of_measurement": "°C",
            "device_class": "temperature",
            "friendly_name": "Living room"
        },
        "last_changed": "2026-11-04T10:12:33.123456+00:00",
        "last_updated": "2026-11-04T10:12:33.123456+00:00"
    }"#;

    fn q(attribute: Option<&str>, unit: Option<&str>) -> HaQuery {
        HaQuery {
            index: 0,
            entity: "sensor.living_room_temperature".into(),
            attribute: attribute.map(Into::into),
            unit: unit.map(Into::into),
        }
    }

    #[test]
    fn a_sensor_state_parses() {
        let e = parse_state(STATE, &q(None, None)).unwrap();
        assert_eq!(e.state, "21.4");
        assert_eq!(e.name, "Living room");
        assert_eq!(e.unit.as_deref(), Some("°C"));
        assert_eq!(e.device_class.as_deref(), Some("temperature"));
        assert_eq!(e.domain, "sensor");
        assert_eq!(e.last_changed, Some(1_793_787_153));
    }

    #[test]
    fn an_attribute_replaces_the_state_and_drops_the_unit() {
        let body = r#"{"entity_id": "climate.hall", "state": "heat",
            "attributes": {"current_temperature": 19.5, "friendly_name": "Hall"}}"#;
        let e = parse_state(body, &q(Some("current_temperature"), None)).unwrap();
        assert_eq!(e.state, "19.5");
        assert_eq!(e.unit, None);
        assert_eq!(e.domain, "climate");
        let e = parse_state(body, &q(Some("current_temperature"), Some("°C"))).unwrap();
        assert_eq!(e.unit.as_deref(), Some("°C"));
        let e = parse_state(body, &q(Some("missing"), None)).unwrap();
        assert_eq!(e.state, "unknown");
    }

    #[test]
    fn no_friendly_name_uses_the_id_and_junk_is_none() {
        let body = r#"{"entity_id": "light.desk", "state": "on", "attributes": {}}"#;
        let e = parse_state(body, &q(None, None)).unwrap();
        assert_eq!(e.name, "light.desk");
        assert_eq!(e.last_changed, None);
        assert_eq!(parse_state("not json", &q(None, None)), None);
        assert_eq!(parse_state("{}", &q(None, None)), None);
    }

    #[test]
    fn urls() {
        assert_eq!(
            state_url("http://ha:8123/", "light.desk"),
            "http://ha:8123/api/states/light.desk"
        );
    }
}
