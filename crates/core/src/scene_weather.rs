//! Open-Meteo roles: weather now, wind compass, air quality.

use crate::anim::{pulse, Secs};
use crate::model::Model;
use crate::scene::{badge, badge_w, icon_at, ring, ring_states, Drawable, Scene, SegState};
use crate::theme::layout::*;
use crate::theme::{eaqi_color, outdoor_color, Color, AMBER, BLUE, DIM_GREY, MOON, OFF, WHITE};

/// Lucide icon for a WMO weather interpretation code.
pub fn weather_icon(code: u16, is_day: bool) -> &'static str {
    match code {
        0 => {
            if is_day {
                "sun"
            } else {
                "moon"
            }
        }
        1 | 2 => {
            if is_day {
                "cloud-sun"
            } else {
                "cloud-moon"
            }
        }
        3 => "cloud",
        45 | 48 => "cloud-fog",
        51..=57 => "cloud-drizzle",
        61..=67 | 80..=82 => "cloud-rain",
        71..=77 | 85 | 86 => "snowflake",
        95..=99 => "cloud-lightning",
        _ => "cloud",
    }
}

/// Colour and micro-loop `(scale, dy, alpha)` per icon.
fn icon_motion(icon: &str, now: Secs) -> (Color, f32, f32, f32) {
    match icon {
        "sun" => (AMBER, 1.0 + 0.06 * pulse(now, 3.5), 0.0, 1.0),
        "moon" => (MOON, 1.0 + 0.04 * pulse(now, 3.5), 0.0, 1.0),
        "cloud-sun" => (AMBER, 1.0, -3.0 * pulse(now, 2.6), 1.0),
        "cloud-moon" => (MOON, 1.0, -3.0 * pulse(now, 2.6), 1.0),
        "cloud" => (WHITE, 1.0, -3.0 * pulse(now, 2.6), 1.0),
        "cloud-fog" => (WHITE, 1.0, 0.0, 0.6 + 0.4 * pulse(now, 3.0)),
        "cloud-drizzle" | "cloud-rain" => (BLUE, 1.0, -3.0 * pulse(now, 1.4), 1.0),
        "snowflake" => (WHITE, 1.0, -3.0 * pulse(now, 3.0), 1.0),
        "cloud-lightning" => (AMBER, 1.0, 0.0, 0.5 + 0.5 * pulse(now, 0.9)),
        _ => (WHITE, 1.0, 0.0, 1.0),
    }
}

pub fn weather_scene(model: &Model, now: Secs) -> Scene {
    let w = model.weather();
    let t = model.smooth_temp(now);
    let color = outdoor_color(t);
    let mut s = Scene::new();
    s.push(ring(
        RING_R,
        ring_states(
            ((t + 10.0) / 50.0 * 100.0).clamp(0.0, 100.0),
            color,
            SEG_N,
            now,
        ),
    ));
    let name = weather_icon(w.code, w.is_day);
    let (icon_color, scale, dy, alpha) = icon_motion(name, now);
    let mut ic = icon_at(name, ICON_CY, ICON_SIZE, icon_color, alpha);
    if let Drawable::Icon {
        scale: sc, dy: d, ..
    } = &mut ic
    {
        *sc = scale;
        *d = dy;
    }
    s.push(ic);
    s.push(badge(BADGE_CY, color, format!("{t:.0}°C")));
    s
}

/// Below this the compass is all dim and the badge says `calm`.
pub const CALM_KMH: f32 = 3.0;

/// A blue arc centred on the from-direction (12 o'clock is north), half-width
/// `3 + speed / 5` segments, fading to the edge; a gust widens it for 0.8 s
/// every 6 s when it beats the mean speed by 30 %.
pub fn wind_states(from_deg: f32, speed_kmh: f32, gust_kmh: f32, now: Secs) -> Vec<SegState> {
    let n = SEG_N as i32;
    if speed_kmh < CALM_KMH {
        return vec![SegState::On(DIM_GREY, 1.0); SEG_N];
    }
    let gusting = gust_kmh > speed_kmh * 1.3 && now.rem_euclid(6.0) < 0.8;
    let speed = if gusting { gust_kmh } else { speed_kmh };
    let half = ((3.0 + speed / 5.0).round() as i32).min(29);
    let centre = ((from_deg / 6.0).round() as i32).rem_euclid(n);
    (0..n)
        .map(|i| {
            let d = ((i - centre).rem_euclid(n)).min((centre - i).rem_euclid(n));
            if d <= half {
                SegState::On(BLUE.mix(OFF, d as f32 / (half as f32 + 1.0)), 1.0)
            } else {
                SegState::Off
            }
        })
        .collect()
}

pub fn wind_scene(model: &Model, now: Secs) -> Scene {
    let w = model.weather();
    let speed = model.smooth_wind(now);
    let mut s = Scene::new();
    s.push(ring(
        RING_R,
        wind_states(w.wind_from_deg, speed, w.gust_kmh, now),
    ));
    s.push(icon_at("wind", ICON_CY, ICON_SIZE, BLUE, 1.0));
    let text = if speed < CALM_KMH {
        "calm".to_string()
    } else {
        format!("{speed:.0} km/h")
    };
    s.push(badge_w(BADGE_CY, BLUE, text, 84.0));
    s
}

pub fn aqi_scene(model: &Model, now: Secs) -> Scene {
    let v = model.smooth_eaqi(now);
    let color = eaqi_color(v);
    let mut s = Scene::new();
    s.push(ring(
        RING_R,
        ring_states(v.clamp(0.0, 100.0), color, SEG_N, now),
    ));
    s.push(icon_at("haze", ICON_CY, ICON_SIZE, color, 1.0));
    s.push(badge_w(BADGE_CY, color, format!("AQI {v:.0}"), 84.0));
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::Event;
    use crate::model::Thresholds;
    use crate::theme::{GREEN, RED};

    fn model(code: u16, is_day: bool, temp: f32, wind: f32, gust: f32, from: f32) -> Model {
        let mut m = Model::new(Thresholds::default());
        m.apply(
            Event::Weather {
                temp_c: temp,
                code,
                is_day,
                wind_kmh: wind,
                gust_kmh: gust,
                wind_from_deg: from,
                at: String::new(),
            },
            0.0,
        );
        m
    }

    fn badge_of(s: &Scene) -> (String, Color, f32) {
        s.items
            .iter()
            .find_map(|d| match d {
                Drawable::Badge {
                    text, stroke, w, ..
                } => Some((text.clone(), *stroke, *w)),
                _ => None,
            })
            .unwrap()
    }

    #[test]
    fn wmo_codes_map_to_icons() {
        assert_eq!(weather_icon(0, true), "sun");
        assert_eq!(weather_icon(0, false), "moon");
        assert_eq!(weather_icon(2, false), "cloud-moon");
        assert_eq!(weather_icon(3, true), "cloud");
        assert_eq!(weather_icon(48, true), "cloud-fog");
        assert_eq!(weather_icon(55, true), "cloud-drizzle");
        assert_eq!(weather_icon(81, true), "cloud-rain");
        assert_eq!(weather_icon(75, true), "snowflake");
        assert_eq!(weather_icon(99, true), "cloud-lightning");
        assert_eq!(weather_icon(123, true), "cloud");
    }

    #[test]
    fn weather_ring_and_badge_follow_the_outdoor_scale() {
        let s = weather_scene(&model(2, true, 18.0, 0.0, 0.0, 0.0), 5.0);
        // (18 + 10) / 50 of 60 = 33.6 -> 34
        assert_eq!(s.lit_count(), 34);
        let (text, stroke, _) = badge_of(&s);
        assert_eq!(text, "18°C");
        assert_eq!(stroke, outdoor_color(18.0));
        assert!(s.items.iter().any(
            |d| matches!(d, Drawable::Icon { name: "cloud-sun", color, .. } if *color == AMBER)
        ));
        let cold = weather_scene(&model(71, true, -12.0, 0.0, 0.0, 0.0), 5.0);
        assert_eq!(cold.lit_count(), 0);
        assert_eq!(badge_of(&cold).1, BLUE);
        let hot = weather_scene(&model(0, true, 40.0, 0.0, 0.0, 0.0), 5.0);
        assert_eq!(hot.lit_count(), 60);
        assert_eq!(badge_of(&hot).1, RED);
    }

    #[test]
    fn wind_arc_is_centred_on_the_from_direction() {
        // from 90° (east): centre segment 15, 23 km/h -> half width 8
        let v = wind_states(90.0, 23.0, 23.0, 1.0);
        assert!(matches!(v[15], SegState::On(c, _) if c == BLUE));
        assert!(matches!(v[7], SegState::On(..)) && matches!(v[23], SegState::On(..)));
        assert_eq!(v[6], SegState::Off);
        assert_eq!(v[24], SegState::Off);
        assert_eq!(v[45], SegState::Off, "the far side is dark");
        // wrap-around near north
        let v = wind_states(357.0, 10.0, 10.0, 1.0);
        assert!(matches!(v[0], SegState::On(c, _) if c == BLUE));
        assert!(matches!(v[59], SegState::On(..)) && matches!(v[1], SegState::On(..)));
        // a gust widens the arc briefly
        let calm_phase = wind_states(90.0, 20.0, 40.0, 3.0);
        let gust_phase = wind_states(90.0, 20.0, 40.0, 6.2);
        let lit = |v: &[SegState]| v.iter().filter(|s| matches!(s, SegState::On(..))).count();
        assert!(lit(&gust_phase) > lit(&calm_phase));
        // calm: everything dim, nothing off
        assert!(wind_states(90.0, 1.0, 1.0, 0.0)
            .iter()
            .all(|s| matches!(s, SegState::On(c, _) if *c == DIM_GREY)));
    }

    #[test]
    fn wind_and_aqi_badges() {
        let s = wind_scene(&model(0, true, 18.0, 23.0, 39.0, 232.0), 5.0);
        assert_eq!(badge_of(&s), ("23 km/h".into(), BLUE, 84.0));
        let calm = wind_scene(&model(0, true, 18.0, 1.0, 2.0, 0.0), 5.0);
        assert_eq!(badge_of(&calm).0, "calm");
        let mut m = Model::new(Thresholds::default());
        m.apply(Event::AirQuality { eaqi: 32.0 }, 0.0);
        let s = aqi_scene(&m, 5.0);
        let (text, stroke, w) = badge_of(&s);
        assert_eq!((text.as_str(), w), ("AQI 32", 84.0));
        assert_eq!(stroke, Color::hex(0x50CCAA));
        assert_eq!(s.lit_count(), 19);
        assert_ne!(stroke, GREEN);
    }
}
