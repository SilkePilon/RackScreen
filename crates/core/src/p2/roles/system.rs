//! Rack hardware and the plain fill-ring roles: thermal, net, ups, wind,
//! rain, aqi, sun, moon, renewable.

use crate::anim::Secs;
use crate::format::{fmt_hhmm, fmt_hm, fmt_in, fmt_runtime};
use crate::model::net_fill;
use crate::model::Model;
use crate::p2::chart::compass_name;
use crate::p2::chart::Card;
use crate::p2::ring::RingSpec;
use crate::p2::roles::short_node;
use crate::p2::screen::P2Screen;
use crate::scene_rain::{current_slots, first_wet_minutes, WET_MM};
use crate::sky::{daylight_secs, moon_days, next_sun_event, sun_progress, SunEvent};
use crate::theme::eaqi_band;
use crate::theme::p2::{AMBER, BLUE, CYAN, GREEN, MOON, NIGHT, TEXT, TEXT3};
use crate::theme::p2::{CRIT, OK, TEXT2, WARN};
use crate::theme::Color;

/// `OK` under `hot - 5`, `WARN` from there, `CRIT` from `hot + 5` (65 / 75 with
/// the default `hot_temp` of 70).
pub fn temp_color(c: f32, hot: f32) -> Color {
    if c >= hot + 5.0 {
        CRIT
    } else if c >= hot - 5.0 {
        WARN
    } else {
        OK
    }
}

pub fn thermal(model: &Model, _now: Secs) -> P2Screen {
    let st = model.state();
    let hot = model.thresholds().hot_temp;
    let hottest = st
        .temps
        .iter()
        .enumerate()
        .max_by(|a, b| a.1 .1.total_cmp(&b.1 .1));
    let (idx, name, max) = match hottest {
        Some((i, (n, c))) => (Some(i), n.as_str(), *c),
        None => (None, "", 0.0),
    };
    let mut s = P2Screen::new("THERMAL", format!("{max:.0}"));
    s.unit = Some("°".into());
    s.ring = RingSpec::fill(((max - 30.0) / 55.0).clamp(0.0, 1.0), temp_color(max, hot));
    s.pill = Some((format!("{name} hottest"), TEXT2));
    s.card = Some(Card::NodeBars {
        values: st.temps.iter().map(|(_, c)| *c).collect(),
        min: 30.0,
        max: 85.0,
        labels: st.temps.iter().map(|(n, _)| short_node(n)).collect(),
        colors: st.temps.iter().map(|(_, c)| temp_color(*c, hot)).collect(),
        highlight: idx,
    });
    s
}

pub fn net(model: &Model, now: Secs) -> P2Screen {
    let n = model.net();
    let total = n.rx_bps + n.tx_bps;
    let hist = model.net_history();
    let mut s = P2Screen::new("NETWORK", format!("{:.0}", total / 1e6));
    s.unit = Some("Mb/s".into());
    s.unit_below = true;
    s.ring = RingSpec::fill(net_fill(total), BLUE);
    s.pill = Some((
        format!("↓ {:.0}  ↑ {:.0}", n.rx_bps / 1e6, n.tx_bps / 1e6),
        TEXT2,
    ));
    let series = hist.series();
    let max = (series.iter().cloned().fold(0.0_f32, f32::max) * 1.1).max(1.0);
    s.card = Some(Card::Spark {
        series,
        prev: hist.prev(),
        scroll: hist.scroll(now),
        min: 0.0,
        max,
        now_idx: None,
        color: BLUE,
        caption: "download · 60 s".into(),
    });
    s
}

pub fn ups(model: &Model, _now: Secs) -> P2Screen {
    let u = model.ups();
    let mut s = P2Screen::new("UPS", format!("{:.0}", u.charge_pct));
    s.unit = Some("%".into());
    s.ring = RingSpec::fill(u.charge_pct / 100.0, if u.on_battery { WARN } else { OK });
    s.pill = Some(if u.on_battery {
        ("On battery".into(), WARN)
    } else {
        ("On mains".into(), OK)
    });
    s.card = Some(Card::Stats(vec![
        (fmt_runtime(u.runtime_secs), "runtime".into(), TEXT),
        (format!("{:.0}%", u.load_pct), "load".into(), TEXT),
    ]));
    s
}

pub fn wind(model: &Model, _now: Secs) -> P2Screen {
    let w = model.weather();
    let mut s = P2Screen::new("WIND", format!("{:.0}", w.wind_kmh));
    s.unit = Some("km/h".into());
    s.unit_below = true;
    s.ring = RingSpec::fill(w.wind_kmh / 60.0, CYAN);
    s.pill = Some((format!("Gusts {:.0} km/h", w.gust_kmh), TEXT2));
    s.card = Some(Card::Compass {
        deg: w.wind_from_deg,
        label: format!(
            "{} · {:.0}°",
            compass_name(w.wind_from_deg),
            w.wind_from_deg
        ),
    });
    s
}

pub fn rain(model: &Model, _now: Secs) -> P2Screen {
    let r = model.rain();
    let slots = current_slots(r.from, &r.mm_per_h, model.unix_now());
    let peak = slots.iter().take(24).cloned().fold(0.0_f32, f32::max);
    let now_mm = slots.first().copied().unwrap_or(0.0);
    let mut s = match (now_mm >= WET_MM, first_wet_minutes(&slots)) {
        (true, _) => {
            let mut s = P2Screen::new("RAIN", format!("{now_mm:.1}"));
            s.unit = Some("mm/h".into());
            s.pill = Some(("Raining now".into(), BLUE));
            s
        }
        (false, Some(mins)) => {
            let mut s = P2Screen::new("RAIN", mins.to_string());
            s.unit = Some("min".into());
            s.pill = Some((format!("until rain · {peak:.1} mm/h"), BLUE));
            s
        }
        (false, None) => {
            let mut s = P2Screen::new("RAIN", "—");
            s.hero_color = TEXT3;
            s.unit = Some("min".into());
            s.pill = Some(("Dry for 2 h".into(), TEXT2));
            s
        }
    };
    s.unit_below = true;
    s.ring = RingSpec::fill((peak / 5.0).clamp(0.0, 1.0), BLUE);
    let mut values: Vec<f32> = slots.iter().take(24).copied().collect();
    values.resize(24, 0.0);
    s.card = Some(Card::Bars {
        values,
        colors: vec![BLUE; 24],
        now: 0,
        past_alpha: 1.0,
        caption: "next 2 h".into(),
    });
    s
}

/// The six EAQI levels, good to extremely poor.
pub const EAQI_BANDS: [u32; 6] = [0x45D6A4, 0x9BD07A, 0xE8C860, 0xF2A65A, 0xF0605D, 0xB07CF0];

pub fn eaqi_name(band: usize) -> &'static str {
    [
        "Good",
        "Fair",
        "Moderate",
        "Poor",
        "Very poor",
        "Extremely poor",
    ][band.min(5)]
}

pub fn aqi(model: &Model, _now: Secs) -> P2Screen {
    let a = model.air();
    let band = eaqi_band(a.eaqi);
    let color = Color::hex(EAQI_BANDS[band]);
    let mut s = P2Screen::new("AIR QUALITY", format!("{:.0}", a.eaqi));
    s.ring = RingSpec::bands(
        EAQI_BANDS.iter().map(|c| Color::hex(*c)).collect(),
        a.eaqi / 120.0,
    );
    s.pill = Some((eaqi_name(band).into(), color));
    let max = (a.hourly.iter().cloned().fold(0.0_f32, f32::max) * 1.1).max(60.0);
    s.card = Some(Card::Spark {
        now_idx: Some(a.hourly.len().saturating_sub(1)),
        series: a.hourly.clone(),
        prev: None,
        scroll: 1.0,
        min: 0.0,
        max,
        color,
        caption: "24 h".into(),
    });
    s
}

pub fn sun(model: &Model, _now: Secs) -> P2Screen {
    let sky = model.sky();
    let unix = model.unix_now();
    let offset = model.utc_offset_secs() as i64;
    let Some((event, at)) = next_sun_event(sky.sunrise, sky.sunset, unix) else {
        let mut s = P2Screen::new("SUN", "—");
        s.hero_color = TEXT3;
        s.pill = Some(("Polar day or night".into(), TEXT2));
        return s;
    };
    let (title, name) = match event {
        SunEvent::Sunset => ("SUNSET", "Sunset"),
        SunEvent::Sunrise => ("SUNRISE", "Sunrise"),
    };
    let mut s = P2Screen::new(title, fmt_hhmm(at + offset));
    s.hero_scale = 0.82;
    let progress = sun_progress(sky.sunrise, sky.sunset, unix).unwrap_or(0.0);
    s.ring = RingSpec::parts(vec![(progress, AMBER), (1.0 - progress, NIGHT)]);
    s.pill = Some((format!("{name} · {}", fmt_in(at - unix)), AMBER));
    let (other_label, other) = match event {
        SunEvent::Sunset => ("sunrise", sky.sunrise),
        SunEvent::Sunrise => ("sunset", sky.sunset),
    };
    s.card = Some(Card::Stats(vec![
        (
            other
                .map(|t| fmt_hhmm(t + offset))
                .unwrap_or_else(|| "—".into()),
            other_label.into(),
            TEXT,
        ),
        (
            daylight_secs(sky.sunrise, sky.sunset)
                .map(fmt_hm)
                .unwrap_or_else(|| "—".into()),
            "daylight".into(),
            TEXT,
        ),
    ]));
    s
}

pub fn moon(model: &Model, _now: Secs) -> P2Screen {
    let sky = model.sky();
    let illum = sky.moon_illumination.clamp(0.0, 1.0);
    let mut s = P2Screen::new("MOON", format!("{:.0}", illum * 100.0));
    s.unit = Some("%".into());
    s.ring = RingSpec::fill(illum, MOON);
    s.pill = Some((sky.moon_phase.name().into(), TEXT2));
    let (full, new) = moon_days(illum, sky.moon_waxing);
    s.card = Some(Card::Stats(vec![
        (format!("{full:.0} d"), "to full".into(), TEXT),
        (format!("{new:.0} d"), "to new".into(), TEXT),
    ]));
    s
}

pub fn renewable(model: &Model, _now: Secs) -> P2Screen {
    let e = model.electricity();
    let mut s = P2Screen::new("RENEWABLE", format!("{:.0}", e.renewable_pct));
    s.unit = Some("%".into());
    s.ring = RingSpec::fill(e.renewable_pct / 100.0, GREEN);
    s.pill = Some((format!("Fossil-free {:.0} %", e.fossil_free_pct), TEXT2));
    let series = model
        .renewable_hourly()
        .values_since(model.local_midnight_unix_hour());
    s.card = Some(Card::Spark {
        now_idx: Some(series.len().saturating_sub(1)),
        series,
        prev: None,
        scroll: 1.0,
        min: 0.0,
        max: 100.0,
        color: GREEN,
        caption: "today".into(),
    });
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::Event;
    use crate::model::Thresholds;
    use crate::p2::ring::RingKind;
    use crate::theme::p2::{CRIT, OK, TEXT2, WARN};

    #[test]
    fn thermal_names_the_hottest_node_and_bars_the_rest() {
        let mut m = Model::new(Thresholds::default());
        m.apply(
            Event::NodeTemps(vec![
                ("hp-elitedesk-800-g5-i7".into(), 46.0),
                ("raspberrypi-5-8gb-1".into(), 54.0),
                ("raspberrypi-5-8gb-2".into(), 68.0),
            ]),
            0.0,
        );
        let s = thermal(&m, 0.0);
        assert_eq!(
            (s.title.as_str(), s.hero.as_str(), s.unit.as_deref()),
            ("THERMAL", "68", Some("°"))
        );
        assert!(
            matches!(s.ring.kind, RingKind::Fill { frac, color } if (frac - 38.0 / 55.0).abs() < 1e-5 && color == WARN)
        );
        assert_eq!(s.pill, Some(("raspberrypi-5-8gb-2 hottest".into(), TEXT2)));
        match &s.card {
            Some(Card::NodeBars {
                values,
                min,
                max,
                labels,
                colors,
                highlight,
            }) => {
                assert_eq!(values, &vec![46.0, 54.0, 68.0]);
                assert_eq!((*min, *max, *highlight), (30.0, 85.0, Some(2)));
                assert_eq!(labels, &vec!["i7".to_string(), "r1".into(), "r2".into()]);
                assert_eq!(colors, &vec![OK, OK, WARN]);
            }
            _ => panic!("node bars"),
        }
        assert_eq!(temp_color(76.0, 70.0), CRIT);
        assert_eq!(temp_color(64.9, 70.0), OK);
    }
    use crate::event::MoonPhase;
    use crate::theme::p2::{AMBER, BLUE, CYAN, GREEN, MOON, NIGHT, TEXT, TEXT3};

    #[test]
    fn net_and_ups() {
        let mut m = Model::new(Thresholds::default());
        m.apply(
            Event::Network {
                rx_bps: 68e6,
                tx_bps: 6e6,
            },
            0.0,
        );
        let s = net(&m, 0.0);
        assert_eq!(
            (
                s.title.as_str(),
                s.hero.as_str(),
                s.unit.as_deref(),
                s.unit_below
            ),
            ("NETWORK", "74", Some("Mb/s"), true)
        );
        assert!(
            matches!(s.ring.kind, RingKind::Fill { frac, color } if (frac - crate::model::net_fill(74e6)).abs() < 1e-6 && color == BLUE)
        );
        assert_eq!(s.pill, Some(("↓ 68  ↑ 6".into(), TEXT2)));
        assert!(
            matches!(&s.card, Some(Card::Spark { series, min, max, caption, .. }) if series == &vec![68.0] && *min == 0.0 && (*max - 74.8).abs() < 1e-3 && caption == "download · 60 s")
        );
        m.apply(
            Event::Ups {
                on_battery: false,
                low_battery: false,
                charge_pct: 100.0,
                load_pct: 7.0,
                runtime_secs: 49 * 60,
            },
            0.0,
        );
        let s = ups(&m, 0.0);
        assert_eq!(
            (s.title.as_str(), s.hero.as_str(), s.unit.as_deref()),
            ("UPS", "100", Some("%"))
        );
        assert!(
            matches!(s.ring.kind, RingKind::Fill { frac, color } if frac == 1.0 && color == OK)
        );
        assert_eq!(s.pill, Some(("On mains".into(), OK)));
        assert!(
            matches!(&s.card, Some(Card::Stats(items)) if items[0] == ("49 min".to_string(), "runtime".to_string(), TEXT) && items[1] == ("7%".to_string(), "load".to_string(), TEXT))
        );
        m.apply(
            Event::Ups {
                on_battery: true,
                low_battery: false,
                charge_pct: 80.0,
                load_pct: 7.0,
                runtime_secs: 38 * 60,
            },
            1.0,
        );
        let s = ups(&m, 1.0);
        assert!(matches!(s.ring.kind, RingKind::Fill { color, .. } if color == WARN));
        assert_eq!(s.pill, Some(("On battery".into(), WARN)));
    }

    fn sky_model(unix: i64) -> Model {
        let mut m = Model::new(Thresholds::default());
        m.set_unix_now(unix);
        m.set_utc_offset_secs(7200);
        m.apply(
            Event::Weather {
                temp_c: 18.0,
                code: 2,
                is_day: true,
                wind_kmh: 23.0,
                gust_kmh: 39.0,
                wind_from_deg: 225.0,
                at: String::new(),
            },
            0.0,
        );
        m.apply(Event::AirQuality { eaqi: 32.0 }, 0.0);
        m.apply(
            Event::AirQualityHistory {
                hourly: vec![30.0, 35.0, 32.0],
            },
            0.0,
        );
        m.apply(
            Event::Sky {
                sunrise: Some(1_788_782_400 - 7 * 3600),
                sunset: Some(1_788_782_400 + 6 * 3600),
                sun_elevation_deg: 45.0,
                moon_illumination: 0.64,
                moon_waxing: true,
                moon_phase: MoonPhase::WaxingGibbous,
            },
            0.0,
        );
        m
    }

    #[test]
    fn wind_and_aqi() {
        let m = sky_model(1_788_782_400);
        let s = wind(&m, 0.0);
        assert_eq!(
            (
                s.title.as_str(),
                s.hero.as_str(),
                s.unit.as_deref(),
                s.unit_below
            ),
            ("WIND", "23", Some("km/h"), true)
        );
        assert!(
            matches!(s.ring.kind, RingKind::Fill { frac, color } if (frac - 23.0 / 60.0).abs() < 1e-5 && color == CYAN)
        );
        assert_eq!(s.pill, Some(("Gusts 39 km/h".into(), TEXT2)));
        assert!(
            matches!(&s.card, Some(Card::Compass { deg, label }) if *deg == 225.0 && label == "SW · 225°")
        );
        let s = aqi(&m, 0.0);
        assert_eq!((s.title.as_str(), s.hero.as_str()), ("AIR QUALITY", "32"));
        assert!(
            matches!(&s.ring.kind, RingKind::Bands { colors, frac } if colors.len() == 6 && (frac - 32.0 / 120.0).abs() < 1e-5)
        );
        assert_eq!(s.pill, Some(("Fair".into(), Color::hex(EAQI_BANDS[1]))));
        assert!(
            matches!(&s.card, Some(Card::Spark { series, now_idx, min, max, caption, .. }) if series == &vec![30.0, 35.0, 32.0] && *now_idx == Some(2) && *min == 0.0 && *max == 60.0 && caption == "24 h")
        );
        assert_eq!(eaqi_name(5), "Extremely poor");
    }

    #[test]
    fn rain_counts_down_or_reports_dry() {
        let mut m = Model::new(Thresholds::default());
        m.set_unix_now(1000);
        let mut slots = vec![0.0f32; 24];
        slots[3] = 0.4;
        slots[4] = 1.8;
        m.apply(
            Event::Rain {
                from: 1000,
                mm_per_h: slots,
            },
            0.0,
        );
        let s = rain(&m, 0.0);
        assert_eq!(
            (
                s.title.as_str(),
                s.hero.as_str(),
                s.unit.as_deref(),
                s.unit_below
            ),
            ("RAIN", "15", Some("min"), true)
        );
        assert!(
            matches!(s.ring.kind, RingKind::Fill { frac, color } if (frac - 1.8 / 5.0).abs() < 1e-5 && color == BLUE)
        );
        assert_eq!(s.pill, Some(("until rain · 1.8 mm/h".into(), BLUE)));
        assert!(
            matches!(&s.card, Some(Card::Bars { values, now, past_alpha, caption, .. }) if values.len() == 24 && values[4] == 1.8 && *now == 0 && *past_alpha == 1.0 && caption == "next 2 h")
        );
        let mut wet = Model::new(Thresholds::default());
        wet.apply(
            Event::Rain {
                from: 0,
                mm_per_h: vec![2.3; 24],
            },
            0.0,
        );
        let s = rain(&wet, 0.0);
        assert_eq!((s.hero.as_str(), s.unit.as_deref()), ("2.3", Some("mm/h")));
        assert_eq!(s.pill, Some(("Raining now".into(), BLUE)));
        let mut dry = Model::new(Thresholds::default());
        dry.apply(
            Event::Rain {
                from: 0,
                mm_per_h: vec![0.0; 24],
            },
            0.0,
        );
        let s = rain(&dry, 0.0);
        assert_eq!((s.hero.as_str(), s.hero_color), ("—", TEXT3));
        assert_eq!(s.pill, Some(("Dry for 2 h".into(), TEXT2)));
        assert!(matches!(s.ring.kind, RingKind::Fill { frac, .. } if frac == 0.0));
    }

    #[test]
    fn sun_moon_and_renewable() {
        // 12:00 UTC = 14:00 local; sunrise 07:00 local, sunset 20:00 local
        let m = sky_model(1_788_782_400);
        let s = sun(&m, 0.0);
        assert_eq!(
            (s.title.as_str(), s.hero.as_str(), s.hero_scale),
            ("SUNSET", "20:00", 0.82)
        );
        assert!(
            matches!(&s.ring.kind, RingKind::Parts(p) if p.len() == 2 && (p[0].0 - 7.0 / 13.0).abs() < 1e-5 && p[0].1 == AMBER && p[1].1 == NIGHT)
        );
        assert_eq!(s.pill, Some(("Sunset · in 6h 00m".into(), AMBER)));
        assert!(
            matches!(&s.card, Some(Card::Stats(items)) if items[0] == ("07:00".to_string(), "sunrise".to_string(), TEXT) && items[1] == ("13h 00m".to_string(), "daylight".to_string(), TEXT))
        );
        let night = sun(&sky_model(1_788_782_400 + 9 * 3600), 0.0);
        assert_eq!(
            (night.title.as_str(), night.hero.as_str()),
            ("SUNRISE", "07:00")
        );
        assert!(matches!(&night.card, Some(Card::Stats(items)) if items[0].1 == "sunset"));
        assert_eq!(night.pill, Some(("Sunrise · in 8h 00m".into(), AMBER)));
        let s = moon(&m, 0.0);
        assert_eq!(
            (s.title.as_str(), s.hero.as_str(), s.unit.as_deref()),
            ("MOON", "64", Some("%"))
        );
        assert!(
            matches!(s.ring.kind, RingKind::Fill { frac, color } if (frac - 0.64).abs() < 1e-5 && color == MOON)
        );
        assert_eq!(s.pill, Some(("Waxing gibbous".into(), TEXT2)));
        assert!(
            matches!(&s.card, Some(Card::Stats(items)) if items[0] == ("6 d".to_string(), "to full".to_string(), TEXT) && items[1] == ("21 d".to_string(), "to new".to_string(), TEXT))
        );
        assert_eq!(MoonPhase::LastQuarter.name(), "Last quarter");
        let mut e = sky_model(1_788_782_400);
        for (h, ren) in [(-3, 55.0), (-2, 60.0), (-1, 70.0), (0, 72.0)] {
            e.set_unix_now(1_788_782_400 + h * 3600);
            e.apply(
                Event::Electricity {
                    zone: "NL".into(),
                    mix_mw: vec![
                        (crate::electricity::Source::Wind, 72.0),
                        (crate::electricity::Source::Gas, 28.0),
                    ],
                    renewable_pct: ren,
                    fossil_free_pct: 82.0,
                    carbon_gco2: 100.0,
                    updated_at: String::new(),
                },
                0.0,
            );
        }
        let s = renewable(&e, 0.0);
        assert_eq!(
            (s.title.as_str(), s.hero.as_str(), s.unit.as_deref()),
            ("RENEWABLE", "72", Some("%"))
        );
        assert!(
            matches!(s.ring.kind, RingKind::Fill { frac, color } if (frac - 0.72).abs() < 1e-5 && color == GREEN)
        );
        assert_eq!(s.pill, Some(("Fossil-free 82 %".into(), TEXT2)));
        assert!(
            matches!(&s.card, Some(Card::Spark { series, now_idx, min, max, caption, .. }) if series == &vec![55.0, 60.0, 70.0, 72.0] && *now_idx == Some(3) && *min == 0.0 && *max == 100.0 && caption == "today")
        );
    }
}
