//! Computed sky roles: the sun dial, the moon phase and the ISS pass countdown.

use crate::anim::{breathe, Secs};
use crate::format::fmt_until;
use crate::model::Model;
use crate::scene::{badge, badge_w, icon_at, ring, ring_states, Drawable, Scene, SegState};
use crate::theme::layout::*;
use crate::theme::{Color, AMBER, GREY, MOON, VIOLET};

pub const DAY: Color = AMBER;
pub const NIGHT: Color = Color::hex(0x1b2a4a);
pub const NOW: Color = Color::hex(0xfff2b0);
/// Seconds of dial per segment: 24 h over 60 segments.
const SEG_SECS: i64 = 1440;
/// Half-width of the twilight blend around sunrise and sunset.
const TWILIGHT_SECS: i64 = 1800;
/// The ISS ring is full this long before a pass and empties toward it.
pub const ISS_HORIZON_SECS: i64 = 12 * 3600;

/// 24-hour dial, midnight at 12 o'clock. `rise` and `set` are seconds of the
/// local day; `polar_day` decides a day without either.
pub fn sun_states(
    local_secs: i64,
    rise: Option<i64>,
    set: Option<i64>,
    polar_day: bool,
    now: Secs,
) -> Vec<SegState> {
    let blend = |centre: i64| -> Color {
        match (rise, set) {
            (Some(r), Some(s)) => {
                let day = centre >= r && centre <= s;
                let base = if day { DAY } else { NIGHT };
                let near_rise = (centre - r).abs() < TWILIGHT_SECS;
                let near_set = (centre - s).abs() < TWILIGHT_SECS;
                if near_rise {
                    NIGHT.mix(
                        DAY,
                        ((centre - r) as f32 / TWILIGHT_SECS as f32 + 1.0) / 2.0,
                    )
                } else if near_set {
                    NIGHT.mix(
                        DAY,
                        ((s - centre) as f32 / TWILIGHT_SECS as f32 + 1.0) / 2.0,
                    )
                } else {
                    base
                }
            }
            _ => {
                if polar_day {
                    DAY
                } else {
                    NIGHT
                }
            }
        }
    };
    let now_seg = (local_secs.rem_euclid(86_400) / SEG_SECS) as usize;
    (0..SEG_N)
        .map(|i| {
            if i == now_seg {
                SegState::On(NOW, breathe(now, 2.4))
            } else {
                SegState::On(blend(i as i64 * SEG_SECS + SEG_SECS / 2), 1.0)
            }
        })
        .collect()
}

pub fn sun_scene(model: &Model, now: Secs) -> Scene {
    let sky = model.sky();
    let unix = model.unix_now();
    let offset = model.utc_offset_secs() as i64;
    let local = unix + offset;
    let local_secs = local.rem_euclid(86_400);
    let day_start = local - local_secs;
    let on_dial = |t: Option<i64>| t.map(|t| (t + offset - day_start).rem_euclid(86_400));
    // Sunrise and sunset are defined at zenith 90.833°, i.e. elevation about
    // -0.83°, so the elevation sign disagrees with them for several minutes
    // either side of each event. Decide from the times and keep the elevation
    // only for the polar case, where there are no times to compare against.
    let daytime = match (sky.sunrise, sky.sunset) {
        (Some(r), Some(s)) => unix >= r && unix < s,
        _ => sky.sun_elevation_deg > 0.0,
    };
    let mut s = Scene::new();
    s.push(ring(
        RING_R,
        sun_states(
            local_secs,
            on_dial(sky.sunrise),
            on_dial(sky.sunset),
            daytime,
            now,
        ),
    ));
    let (icon, target) = if daytime {
        ("sunset", sky.sunset)
    } else {
        ("sunrise", sky.sunrise)
    };
    s.push(icon_at(icon, ICON_CY, ICON_SIZE, AMBER, 1.0));
    let text = match target {
        Some(t) if t >= unix => fmt_until(t - unix),
        _ => "--".to_string(),
    };
    s.push(badge(BADGE_CY, AMBER, text));
    s
}

/// Illuminated fraction as lit segments: clockwise from the top while waxing,
/// counter-clockwise while waning.
pub fn moon_states(illumination: f32, waxing: bool, now: Secs) -> Vec<SegState> {
    let v = ring_states(illumination.clamp(0.0, 1.0) * 100.0, MOON, SEG_N, now);
    if waxing {
        return v;
    }
    let n = v.len();
    (0..n).map(|i| v[(n - i) % n]).collect()
}

pub fn moon_scene(model: &Model, now: Secs) -> Scene {
    let sky = model.sky();
    let mut s = Scene::new();
    s.push(ring(
        RING_R,
        moon_states(sky.moon_illumination, sky.moon_waxing, now),
    ));
    s.push(icon_at("moon", ICON_CY, ICON_SIZE, MOON, 1.0));
    let odd = ((now / 5.0).floor() as i64).rem_euclid(2) == 1;
    let text = if odd {
        sky.moon_phase.label().to_string()
    } else {
        format!("{:.0}%", sky.moon_illumination * 100.0)
    };
    let mut b = badge_w(BADGE_CY, MOON, text, 84.0);
    if let Drawable::Badge { alpha, .. } = &mut b {
        *alpha = (((now - (now / 5.0).floor() * 5.0) / 0.25) as f32).min(1.0);
    }
    s.push(b);
    s
}

/// Ring fill before a pass: full twelve hours out, empty at the start.
pub fn iss_fill(start: i64, unix_now: i64) -> f32 {
    ((start - unix_now).clamp(0, ISS_HORIZON_SECS) as f32 / ISS_HORIZON_SECS as f32) * 100.0
}

pub fn iss_scene(model: &Model, now: Secs) -> Scene {
    let unix = model.unix_now();
    let mut s = Scene::new();
    let (states, text, stroke) = match model.iss().pass {
        Some(p) if unix < p.start => {
            let mut v = ring_states(iss_fill(p.start, unix), VIOLET, SEG_N, now);
            if !p.visible {
                for st in v.iter_mut() {
                    if let SegState::On(c, a) = *st {
                        *st = SegState::On(c, a * 0.4);
                    }
                }
            }
            (v, fmt_until(p.start - unix), VIOLET)
        }
        Some(p) if unix < p.end => {
            let a = breathe(now, 1.2) * if p.visible { 1.0 } else { 0.4 };
            (
                vec![SegState::On(VIOLET, a); SEG_N],
                format!("{:.0}°", p.max_elevation_deg),
                VIOLET,
            )
        }
        _ => (vec![SegState::Off; SEG_N], "--".to_string(), GREY),
    };
    s.push(ring(RING_R, states));
    s.push(icon_at("satellite", ICON_CY, ICON_SIZE, VIOLET, 1.0));
    s.push(badge(BADGE_CY, stroke, text));
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{Event, IssPass, MoonPhase};
    use crate::model::Thresholds;

    fn lit(v: &[SegState]) -> usize {
        v.iter().filter(|s| matches!(s, SegState::On(..))).count()
    }

    #[test]
    fn sun_dial_lights_day_amber_night_blue_and_now_bright() {
        // sunrise 07:00, sunset 20:00, now 14:07
        let v = sun_states(
            14 * 3600 + 7 * 60,
            Some(7 * 3600),
            Some(20 * 3600),
            true,
            0.6,
        );
        assert_eq!(v.len(), 60);
        assert!(matches!(v[0], SegState::On(c, _) if c == NIGHT), "midnight");
        assert!(matches!(v[30], SegState::On(c, _) if c == DAY), "noon");
        assert!(
            matches!(v[35], SegState::On(c, a) if c == NOW && a < 1.0),
            "14:07 breathes"
        );
        assert!(matches!(v[57], SegState::On(c, _) if c == NIGHT), "23:00");
        // twilight blends around sunrise: segment 17 is 06:48..07:12
        assert!(matches!(v[17], SegState::On(c, _) if c != DAY && c != NIGHT));
        // polar cases
        assert!(sun_states(0, None, None, true, 0.0)[30].eq(&SegState::On(DAY, 1.0)));
        assert!(sun_states(0, None, None, false, 0.0)[30].eq(&SegState::On(NIGHT, 1.0)));
    }

    fn sky_model(unix: i64, offset: i32, rise: i64, set: i64, elevation: f32) -> Model {
        let mut m = Model::new(Thresholds::default());
        m.set_unix_now(unix);
        m.set_utc_offset_secs(offset);
        m.apply(
            Event::Sky {
                sunrise: Some(rise),
                sunset: Some(set),
                sun_elevation_deg: elevation,
                moon_illumination: 0.63,
                moon_waxing: false,
                moon_phase: MoonPhase::WaningGibbous,
            },
            0.0,
        );
        m
    }

    fn badge_of(s: &Scene) -> (String, Color) {
        s.items
            .iter()
            .find_map(|d| match d {
                Drawable::Badge { text, stroke, .. } => Some((text.clone(), *stroke)),
                _ => None,
            })
            .unwrap()
    }

    fn icon_of(s: &Scene) -> &'static str {
        s.items
            .iter()
            .find_map(|d| match d {
                Drawable::Icon { name, .. } => Some(*name),
                _ => None,
            })
            .unwrap()
    }

    #[test]
    fn sun_scene_counts_down_to_sunset_by_day_and_sunrise_by_night() {
        // 2026-09-07 12:00 UTC, Amsterdam (+2): sunset 20:00 local = 18:00 UTC
        let day = 1_788_782_400;
        let m = sky_model(day, 7200, day - 5 * 3600, day + 6 * 3600, 40.0);
        let s = sun_scene(&m, 1.0);
        assert_eq!(icon_of(&s), "sunset");
        assert_eq!(badge_of(&s).0, "-6h00");
        let night = sky_model(
            day + 12 * 3600,
            7200,
            day + 19 * 3600,
            day + 6 * 3600,
            -20.0,
        );
        let s = sun_scene(&night, 1.0);
        assert_eq!(icon_of(&s), "sunrise");
        assert_eq!(badge_of(&s).0, "-7h00");
        let mut none = sky_model(day, 7200, 0, 0, -20.0);
        none.apply(
            Event::Sky {
                sunrise: None,
                sunset: None,
                sun_elevation_deg: -20.0,
                moon_illumination: 0.0,
                moon_waxing: true,
                moon_phase: MoonPhase::New,
            },
            0.0,
        );
        assert_eq!(badge_of(&sun_scene(&none, 1.0)).0, "--");
    }

    #[test]
    fn sun_scene_uses_the_times_not_the_elevation_around_the_edges() {
        let day = 1_788_782_400;
        let rise = day - 5 * 3600;
        let set = day + 6 * 3600;
        // five minutes before sunset the sun is already below the horizon
        let m = sky_model(set - 300, 7200, rise, set, -0.5);
        assert_eq!(icon_of(&sun_scene(&m, 1.0)), "sunset");
        assert_eq!(badge_of(&sun_scene(&m, 1.0)).0, "-5m");
        // one minute after sunrise it is still below the horizon, but it is day
        let m = sky_model(rise + 60, 7200, rise, set, -0.7);
        let s = sun_scene(&m, 1.0);
        assert_eq!(icon_of(&s), "sunset");
        assert_eq!(badge_of(&s).0, fmt_until(set - (rise + 60)));
        // no times at all: the elevation still decides
        let mut polar = sky_model(day, 7200, rise, set, 5.0);
        polar.apply(
            Event::Sky {
                sunrise: None,
                sunset: None,
                sun_elevation_deg: 5.0,
                moon_illumination: 0.0,
                moon_waxing: true,
                moon_phase: MoonPhase::New,
            },
            0.0,
        );
        assert_eq!(icon_of(&sun_scene(&polar, 1.0)), "sunset");
    }

    #[test]
    fn moon_ring_direction_and_badge_alternation() {
        let waxing = moon_states(0.63, true, 5.0);
        assert_eq!(lit(&waxing), 38);
        assert!(matches!(waxing[0], SegState::On(..)) && matches!(waxing[37], SegState::On(..)));
        assert_eq!(waxing[38], SegState::Off);
        let waning = moon_states(0.63, false, 5.0);
        assert_eq!(lit(&waning), 38);
        assert!(matches!(waning[0], SegState::On(..)), "the top stays lit");
        assert!(matches!(waning[59], SegState::On(..)) && matches!(waning[23], SegState::On(..)));
        assert_eq!(waning[22], SegState::Off);
        assert_eq!(waning[1], SegState::Off, "nothing clockwise of the top");
        let m = sky_model(0, 0, 0, 0, 0.0);
        assert_eq!(badge_of(&moon_scene(&m, 2.5)).0, "63%");
        assert_eq!(badge_of(&moon_scene(&m, 7.5)).0, "waning");
    }

    fn iss_model(unix: i64, pass: Option<IssPass>) -> Model {
        let mut m = Model::new(Thresholds::default());
        m.set_unix_now(unix);
        m.apply(Event::IssPass(pass), 0.0);
        m
    }

    #[test]
    fn iss_ring_empties_toward_the_pass_and_fills_during_it() {
        assert_eq!(iss_fill(1_000, 1_000), 0.0);
        assert_eq!(iss_fill(1_000 + 6 * 3600, 1_000), 50.0);
        assert_eq!(iss_fill(1_000 + 48 * 3600, 1_000), 100.0);
        let pass = IssPass {
            start: 10_000,
            end: 10_400,
            max_elevation_deg: 62.0,
            visible: true,
        };
        let before = iss_scene(&iss_model(10_000 - 42 * 60, Some(pass)), 5.0);
        assert_eq!(badge_of(&before), ("-42m".into(), VIOLET));
        assert_eq!(before.lit_count(), 4, "42 min of 12 h");
        let during = iss_scene(&iss_model(10_100, Some(pass)), 5.0);
        assert_eq!(badge_of(&during).0, "62°");
        assert_eq!(during.lit_count(), 60);
        let after = iss_scene(&iss_model(11_000, Some(pass)), 5.0);
        assert_eq!(badge_of(&after), ("--".into(), GREY));
        assert_eq!(after.lit_count(), 0);
        assert_eq!(badge_of(&iss_scene(&iss_model(0, None), 5.0)).0, "--");
        // a pass that will not be visible is drawn dim
        let dim = iss_scene(
            &iss_model(
                10_000 - 6 * 3600,
                Some(IssPass {
                    visible: false,
                    ..pass
                }),
            ),
            5.0,
        );
        let alphas: Vec<f32> = dim
            .items
            .iter()
            .find_map(|d| match d {
                Drawable::Ring { states, .. } => Some(
                    states
                        .iter()
                        .filter_map(|s| match s {
                            SegState::On(_, a) => Some(*a),
                            _ => None,
                        })
                        .collect(),
                ),
                _ => None,
            })
            .unwrap();
        assert!(alphas.iter().all(|a| *a <= 0.4));
        assert_eq!(icon_of(&dim), "satellite");
    }
}
