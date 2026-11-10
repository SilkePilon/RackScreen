//! The weather role: the condition, the day's range and the next rain.

use crate::anim::Secs;
use crate::model::Model;
use crate::p2::chart::Card;
use crate::p2::ring::RingSpec;
use crate::p2::screen::P2Screen;
use crate::theme::p2::{AMBER, CYAN, TEXT2};

/// A short name for a WMO weather code (the screen's `title`).
pub fn condition(code: u16) -> &'static str {
    match code {
        0 => "CLEAR",
        1 => "MAINLY CLEAR",
        2 => "PARTLY CLOUDY",
        3 => "OVERCAST",
        45 | 48 => "FOG",
        51..=57 => "DRIZZLE",
        61..=67 => "RAIN",
        71..=77 => "SNOW",
        80..=82 => "SHOWERS",
        85 | 86 => "SNOW SHOWER", // "Snow showers · Dry next 12 h" overflows the pill
        95..=99 => "THUNDER",
        _ => "WEATHER",
    }
}

/// The condition in sentence case for the pill: "Partly cloudy".
pub fn condition_name(code: u16) -> String {
    let lower = condition(code).to_lowercase();
    let mut c = lower.chars();
    c.next()
        .map(|f| f.to_uppercase().chain(c).collect())
        .unwrap_or_default()
}

pub fn weather(model: &Model, _now: Secs) -> P2Screen {
    let w = model.weather();
    let (lo, hi) = match (w.day_min_c, w.day_max_c) {
        (Some(a), Some(b)) if b > a => (a, b),
        _ => (w.temp_c - 5.0, w.temp_c + 5.0),
    };
    let mut s = P2Screen::new(condition(w.code), format!("{:.0}", w.temp_c));
    s.unit = Some("°".into());
    s.ring = RingSpec::fill((w.temp_c - lo) / (hi - lo), AMBER);
    s.ends = Some((format!("{lo:.0}°"), format!("{hi:.0}°")));
    let hour = model.local_hour();
    let (hint, color) = match w.hourly_precip_mm.iter().position(|p| *p >= 0.1) {
        Some(0) => ("Raining now".to_string(), CYAN),
        Some(i) => (format!("Rain {:02}:00", (hour + i) % 24), CYAN),
        None => ("Dry next 12 h".to_string(), TEXT2),
    };
    let name = condition_name(w.code);
    let wet = ["rain", "drizzle", "shower", "storm", "snow"]
        .iter()
        .any(|k| name.to_lowercase().contains(k));
    // "Rain · Raining now" says it twice: a wet condition already says it is raining
    // the generic fallback has nothing to say: the hint alone
    let text = if hint == "Raining now" && wet {
        name
    } else if condition(w.code) == "WEATHER" {
        hint
    } else {
        format!("{name} · {hint}")
    };
    s.pill = Some((text, color));
    s.card = Some(Card::Spark {
        series: w
            .hourly_temp_c
            .iter()
            .copied()
            .filter(|t| t.is_finite())
            .collect(),
        prev: None,
        scroll: 1.0,
        min: lo - 2.0,
        max: hi + 2.0,
        now_idx: Some(0),
        color: AMBER,
        caption: "next 12 h".into(),
    });
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::Event;
    use crate::model::Thresholds;
    use crate::p2::ring::RingKind;
    use crate::theme::p2::{AMBER, CYAN, TEXT2};

    fn weather_model(code: u16, temp: f32, precip: Vec<f32>) -> Model {
        let mut m = Model::new(Thresholds::default());
        m.set_unix_now(1_788_782_400); // 12:00 UTC
        m.set_utc_offset_secs(7200); // 14:00 local
        m.apply(
            Event::Weather {
                temp_c: temp,
                code,
                is_day: true,
                wind_kmh: 8.0,
                gust_kmh: 15.0,
                wind_from_deg: 225.0,
                at: String::new(),
            },
            0.0,
        );
        let n = precip.len();
        m.apply(
            Event::Forecast {
                hourly_temp_c: (0..n).map(|i| temp + i as f32 * 0.5).collect(),
                hourly_precip_mm: precip,
                day_min_c: 11.0,
                day_max_c: 21.0,
            },
            0.0,
        );
        m
    }

    #[test]
    fn weather_titles_the_condition_scales_the_day_and_hints_rain() {
        let m = weather_model(
            2,
            17.0,
            vec![0.0, 0.0, 0.0, 0.0, 1.1, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        );
        let s = weather(&m, 0.0);
        assert_eq!(
            (s.title.as_str(), s.hero.as_str(), s.unit.as_deref()),
            ("PARTLY CLOUDY", "17", Some("°"))
        );
        assert!(
            matches!(s.ring.kind, RingKind::Fill { frac, color } if (frac - 0.6).abs() < 1e-5 && color == AMBER)
        );
        assert_eq!(s.ends, Some(("11°".into(), "21°".into())));
        assert_eq!(s.pill, Some(("Partly cloudy · Rain 18:00".into(), CYAN)));
        assert!(
            matches!(&s.card, Some(Card::Spark { series, now_idx, min, max, caption, .. }) if series.len() == 12 && *now_idx == Some(0) && *min == 9.0 && *max == 23.0 && caption == "next 12 h")
        );
        let dry = weather(&weather_model(0, 24.0, vec![0.0; 12]), 0.0);
        assert_eq!(dry.title, "CLEAR");
        assert_eq!(dry.pill, Some(("Clear · Dry next 12 h".into(), TEXT2)));
        let wet = weather(&weather_model(61, 9.0, vec![2.0; 12]), 0.0);
        assert_eq!(
            wet.pill,
            Some(("Rain".into(), CYAN)),
            "a wet condition says it once"
        );
        assert_eq!(condition_name(85), "Snow shower");
        assert_eq!(condition_name(1), "Mainly clear");
        assert_eq!(condition(95), "THUNDER");
        assert_eq!(condition(48), "FOG");
        assert_eq!(condition(123), "WEATHER");
        let mut bare = Model::new(Thresholds::default());
        bare.apply(
            Event::Weather {
                temp_c: 5.0,
                code: 3,
                is_day: false,
                wind_kmh: 0.0,
                gust_kmh: 0.0,
                wind_from_deg: 0.0,
                at: String::new(),
            },
            0.0,
        );
        let s = weather(&bare, 0.0);
        assert_eq!(
            s.ends,
            Some(("0°".into(), "10°".into())),
            "no daily range: ±5 around now"
        );
        assert!(matches!(&s.card, Some(Card::Spark { series, .. }) if series.is_empty()));
    }

    #[test]
    fn every_weather_pill_fits_its_slot() {
        use crate::p2::font::{measure, FontId};
        use crate::p2::layout::{PILL_MAX_TEXT_W, PILL_PX};
        // every WMO code (0–99) that has a name of its own
        for code in (0..=99).filter(|c| condition(*c) != "WEATHER") {
            let name = condition_name(code);
            for hint in ["Rain 23:00", "Dry next 12 h"] {
                let text = format!("{name} · {hint}");
                let tw = measure(FontId::SemiBold, PILL_PX, &text, 0.0);
                assert!(
                    tw <= PILL_MAX_TEXT_W,
                    "{text:?} is {tw:.1} px, wider than {PILL_MAX_TEXT_W}"
                );
            }
        }
    }

    #[test]
    fn an_unknown_code_shows_only_the_hint() {
        let s = weather(&weather_model(123, 10.0, vec![0.0; 12]), 0.0);
        assert_eq!(s.pill, Some(("Dry next 12 h".into(), TEXT2)));
        let s = weather(&weather_model(123, 10.0, vec![0.0, 0.0, 1.0]), 0.0);
        assert_eq!(s.pill, Some(("Rain 16:00".into(), CYAN)));
    }
}
