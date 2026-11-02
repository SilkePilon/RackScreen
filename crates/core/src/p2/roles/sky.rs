//! Sky roles with a forecast: weather and the ISS pass.

use crate::anim::Secs;
use crate::format::{fmt_hhmm, fmt_mmss};
use crate::model::Model;
use crate::p2::chart::Card;
use crate::p2::ring::RingSpec;
use crate::p2::screen::P2Screen;
use crate::theme::p2::{AMBER, CYAN, TEXT, TEXT2, TEXT3};

/// The curved title for a WMO weather code.
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
        85 | 86 => "SNOW SHOWERS",
        95..=99 => "THUNDER",
        _ => "WEATHER",
    }
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
    s.pill = Some(match w.hourly_precip_mm.iter().position(|p| *p >= 0.1) {
        Some(0) => ("Raining now".into(), CYAN),
        Some(i) => (format!("Rain from {:02}:00", (hour + i) % 24), CYAN),
        None => ("Dry next 12 h".into(), TEXT2),
    });
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

pub fn iss(model: &Model, _now: Secs) -> P2Screen {
    let unix = model.unix_now();
    let offset = model.utc_offset_secs() as i64;
    let visible = |v: bool, el: f32| {
        if v {
            (format!("Visible · max {el:.0}°"), CYAN)
        } else {
            (format!("Not visible · max {el:.0}°"), TEXT2)
        }
    };
    match model.iss().pass {
        Some(p) if unix < p.start => {
            let until = p.start - unix;
            let mut s = P2Screen::new("ISS PASS IN", fmt_mmss(until));
            s.hero_scale = 0.82;
            s.ring = RingSpec::fill(1.0 - (until as f32 / 5400.0).clamp(0.0, 1.0), CYAN);
            s.pill = Some(visible(p.visible, p.max_elevation_deg));
            s.card = Some(Card::Stats(vec![
                (fmt_hhmm(p.start + offset), "start".into(), TEXT),
                (
                    format!("{} min", (p.end - p.start + 30) / 60),
                    "duration".into(),
                    TEXT,
                ),
            ]));
            s
        }
        Some(p) if unix < p.end => {
            let mut s = P2Screen::new("ISS OVERHEAD", format!("{:.0}", p.max_elevation_deg));
            s.unit = Some("°".into());
            s.ring = RingSpec::fill(1.0, CYAN);
            s.pill = Some(if p.visible {
                ("Visible now".into(), CYAN)
            } else {
                ("Not visible".into(), TEXT2)
            });
            s.card = Some(Card::Stats(vec![
                (fmt_mmss(p.end - unix), "ends in".into(), TEXT),
                (
                    format!("{} min", (p.end - p.start + 30) / 60),
                    "duration".into(),
                    TEXT,
                ),
            ]));
            s
        }
        _ => {
            let mut s = P2Screen::new("ISS PASS IN", "—");
            s.hero_color = TEXT3;
            s.pill = Some(("No pass in 24 h".into(), TEXT2));
            s
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{Event, IssPass};
    use crate::model::Thresholds;
    use crate::p2::ring::RingKind;
    use crate::theme::p2::{AMBER, CYAN, TEXT2, TEXT3};

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
        assert_eq!(s.pill, Some(("Rain from 18:00".into(), CYAN)));
        assert!(
            matches!(&s.card, Some(Card::Spark { series, now_idx, min, max, caption, .. }) if series.len() == 12 && *now_idx == Some(0) && *min == 9.0 && *max == 23.0 && caption == "next 12 h")
        );
        let dry = weather(&weather_model(0, 24.0, vec![0.0; 12]), 0.0);
        assert_eq!(dry.title, "CLEAR");
        assert_eq!(dry.pill, Some(("Dry next 12 h".into(), TEXT2)));
        let wet = weather(&weather_model(61, 9.0, vec![2.0; 12]), 0.0);
        assert_eq!(wet.pill, Some(("Raining now".into(), CYAN)));
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

    fn iss_model(unix: i64, pass: Option<IssPass>) -> Model {
        let mut m = Model::new(Thresholds::default());
        m.set_unix_now(unix);
        m.set_utc_offset_secs(7200);
        m.apply(Event::IssPass(pass), 0.0);
        m
    }

    #[test]
    fn iss_counts_down_shows_the_pass_and_rests() {
        let pass = IssPass {
            start: 10_000,
            end: 10_372,
            max_elevation_deg: 62.0,
            visible: true,
        };
        let s = iss(&iss_model(10_000 - 14 * 60 - 32, Some(pass)), 0.0);
        assert_eq!(
            (s.title.as_str(), s.hero.as_str(), s.hero_scale),
            ("ISS PASS IN", "14:32", 0.82)
        );
        assert!(
            matches!(s.ring.kind, RingKind::Fill { frac, color } if (frac - (1.0 - 872.0 / 5400.0)).abs() < 1e-4 && color == CYAN)
        );
        assert_eq!(s.pill, Some(("Visible · max 62°".into(), CYAN)));
        assert!(
            matches!(&s.card, Some(Card::Stats(items)) if items[0] == ("04:46".to_string(), "start".to_string(), TEXT) && items[1] == ("6 min".to_string(), "duration".to_string(), TEXT))
        );
        let during = iss(&iss_model(10_100, Some(pass)), 0.0);
        assert_eq!(
            (
                during.title.as_str(),
                during.hero.as_str(),
                during.unit.as_deref()
            ),
            ("ISS OVERHEAD", "62", Some("°"))
        );
        assert!(matches!(during.ring.kind, RingKind::Fill { frac, .. } if frac == 1.0));
        assert_eq!(during.pill, Some(("Visible now".into(), CYAN)));
        let dim = iss(
            &iss_model(
                5_000,
                Some(IssPass {
                    visible: false,
                    ..pass
                }),
            ),
            0.0,
        );
        assert_eq!(dim.pill, Some(("Not visible · max 62°".into(), TEXT2)));
        let none = iss(&iss_model(0, None), 0.0);
        assert_eq!((none.hero.as_str(), none.hero_color), ("—", TEXT3));
        assert!(matches!(none.ring.kind, RingKind::Empty));
        assert_eq!(none.pill, Some(("No pass in 24 h".into(), TEXT2)));
        assert!(none.card.is_none());
    }
}
