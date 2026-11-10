//! The ISS screen: the Earth as a globe that follows the station, its
//! ground track, the standard ring filling toward the next pass and a pill
//! with the countdown. Everything on the globe is derived from the clock in
//! whole seconds, so the screen changes at most once a second.

use crate::anim::Secs;
use crate::event::{IssPass, IssTrack};
use crate::format::fmt_hms;
use crate::globe::{near_side_track, subsolar, Camera};
use crate::model::Model;
use crate::p2::ring::{RingKind, RingSpec};
use crate::p2::text::{pill, top_icon};
use crate::p2::ui::ScreenUi;
use crate::scene::{Drawable, Scene};
use crate::theme::layout::{CX, CY};
use crate::theme::p2::{CYAN, INK, TEXT, TEXT2};
use crate::theme::{Color, Role, BLACK, WHITE};

pub const GLOBE_R: f32 = 95.0;
/// The pill sits low on the globe, clear of the station.
pub const PILL_CY: f32 = 190.0;
/// The ring is full when the pass starts and empty this long before it.
pub const RING_SPAN_SECS: f32 = 5400.0;
/// The solid track behind the station and the dashed one ahead (one orbit).
pub const PAST_SECS: i64 = 50 * 60;
pub const AHEAD_SECS: i64 = 93 * 60;
/// Spacing of the track's points in time: about 3 px on screen.
pub const TRACK_STEP_SECS: i64 = 30;
pub const MARKER_PX: f32 = 20.0;
pub const MARKER_OUTLINE: f32 = 1.5;
pub const HOME_R: f32 = 2.6;
/// The home dot fades in and out over this long.
pub const HOME_FADE_SECS: Secs = 0.5;
/// A ring step smaller than this is shown at once; only a jump glides.
const RING_SNAP: f32 = 0.002;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    /// The next pass starts in this many seconds.
    Before(i64),
    During,
    NoPass,
}

pub fn phase(pass: Option<&IssPass>, unix: i64) -> Phase {
    match pass {
        Some(p) if unix < p.start => Phase::Before(p.start - unix),
        Some(p) if unix < p.end => Phase::During,
        _ => Phase::NoPass,
    }
}

/// The pill's text and colour.
pub fn pill_text(pass: Option<&IssPass>, unix: i64) -> (String, Color) {
    let max = pass.map_or(0.0, |p| p.max_elevation_deg);
    match phase(pass, unix) {
        Phase::Before(until) => (format!("{} · max {max:.0}°", fmt_hms(until as f64)), TEXT),
        Phase::During => (format!("Overhead · max {max:.0}°"), CYAN),
        Phase::NoPass => ("No pass in 24 h".into(), TEXT2),
    }
}

/// The ring: white, filling over the last 90 minutes before a pass, full
/// during it.
pub fn ring(pass: Option<&IssPass>, unix: i64) -> RingSpec {
    let frac = match phase(pass, unix) {
        Phase::Before(until) => 1.0 - (until as f32 / RING_SPAN_SECS).clamp(0.0, 1.0),
        Phase::During => 1.0,
        Phase::NoPass => 0.0,
    };
    if frac > 0.0 {
        RingSpec::fill(frac, TEXT)
    } else {
        RingSpec::empty()
    }
}

/// The station can be seen by eye from home right now.
pub fn visible_now(pass: Option<&IssPass>, unix: i64) -> bool {
    pass.and_then(|p| p.vis)
        .is_some_and(|(a, b)| unix >= a && unix < b)
}

fn to_screen(p: (f32, f32)) -> (f32, f32) {
    (CX + GLOBE_R * p.0, CY - GLOBE_R * p.1)
}

/// The globe and what moves with it at `unix`: the globe first, then the
/// track ahead, the track behind over its shadow, and the station.
fn globe_layer(track: &IssTrack, unix: i64) -> Vec<Drawable> {
    let Some(iss) = track.at(unix) else {
        return Vec::new();
    };
    let cam = Camera::following(iss);
    let (sun_lat, sun_lon) = subsolar(unix);
    let walk = |dir: i64, max: i64| -> Vec<(f32, f32)> {
        near_side_track(track, &cam, unix, dir, max, TRACK_STEP_SECS)
            .into_iter()
            .map(to_screen)
            .collect()
    };
    let (ahead, behind) = (walk(1, AHEAD_SECS), walk(-1, PAST_SECS));
    let (x, y, _) = cam.project(iss);
    let (mx, my) = to_screen((x as f32, y as f32));
    vec![
        Drawable::Globe {
            cam_lat: cam.lat as f32,
            cam_lon: cam.lon as f32,
            sun_lat: sun_lat as f32,
            sun_lon: sun_lon as f32,
            r: GLOBE_R,
        },
        Drawable::Dashed {
            points: ahead,
            width: 1.5,
            on: 3.0,
            off: 4.0,
            color: WHITE,
            alpha: 0.7,
        },
        Drawable::Polyline {
            points: behind.clone(),
            width: 4.0,
            color: BLACK,
            alpha: 0.4,
        },
        Drawable::Polyline {
            points: behind,
            width: 2.0,
            color: WHITE,
            alpha: 1.0,
        },
        Drawable::IconOutline {
            name: Role::Iss.icon(),
            cx: mx,
            cy: my,
            size: MARKER_PX,
            width: MARKER_OUTLINE,
            color: INK,
            alpha: 0.85,
        },
        Drawable::Icon {
            name: Role::Iss.icon(),
            cx: mx,
            cy: my,
            size: MARKER_PX,
            color: WHITE,
            alpha: 1.0,
            scale: 1.0,
            dy: 0.0,
        },
    ]
}

/// The home dot at `alpha`: white with a dark ring, where the observer is.
fn home_dot(track: &IssTrack, unix: i64, alpha: f32) -> Vec<Drawable> {
    let Some(iss) = track.at(unix) else {
        return Vec::new();
    };
    let cam = Camera::following(iss);
    let (x, y, z) = cam.project(crate::globe::unit(track.home.0 as f64, track.home.1 as f64));
    if z <= 0.0 {
        return Vec::new();
    }
    let (cx, cy) = to_screen((x as f32, y as f32));
    let dot = |r: f32, color: Color, a: f32| Drawable::Ellipse {
        cx,
        cy,
        rx: r,
        ry: r,
        color,
        alpha: a * alpha,
    };
    vec![dot(HOME_R + 1.0, INK, 0.9), dot(HOME_R, WHITE, 1.0)]
}

pub fn iss_scene(model: &Model, now: Secs, ui: &mut ScreenUi) -> Scene {
    let unix = model.unix_now();
    let state = model.iss();
    let pass = state.pass.as_ref();
    let mut scene = Scene::new();
    // the globe changes once a second at most, and with it the base
    let layer: Vec<Drawable> = match &state.track {
        Some(track) => ui
            .memo((unix, track.t0), || globe_layer(track, unix))
            .to_vec(),
        None => Vec::new(),
    };
    let mut layer = layer.into_iter();
    scene.items.extend(layer.next());
    let spec = ring(pass, unix);
    scene.items.extend(spec.track());
    scene.push(top_icon(Role::Iss.icon()));
    scene.base_len = scene.items.len();
    // the fill creeps a tenth of a pixel a second: that is shown as it is,
    // with the rest of the screen; a jump (a new pass) glides
    let spec = match spec.kind {
        RingKind::Fill { frac, color } => {
            let g = ui.glide(("ring", 0), frac, now);
            let frac = if (g - frac).abs() < RING_SNAP {
                frac
            } else {
                g
            };
            RingSpec::fill(frac, color)
        }
        _ => {
            ui.glide(("ring", 0), 0.0, now);
            spec
        }
    };
    scene.items.extend(spec.overlay());
    scene.items.extend(layer);
    let target = if visible_now(pass, unix) { 1.0 } else { 0.0 };
    let alpha = ui.glide_with(("home", 0), target, now, HOME_FADE_SECS);
    if alpha > 0.01 {
        if let Some(track) = &state.track {
            scene.items.extend(home_dot(track, unix, alpha));
        }
    }
    let (text, color) = pill_text(pass, unix);
    let pill = pill(ui, &text, color, PILL_CY, now);
    // the pill's 30 % fill is meant over black, not over the globe
    if let Some(Drawable::RoundRect { x, y, w, h, r, .. }) = pill.first() {
        scene.push(Drawable::RoundRect {
            x: *x,
            y: *y,
            w: *w,
            h: *h,
            r: *r,
            color: BLACK,
            alpha: 1.0,
        });
    }
    scene.items.extend(pill);
    scene
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::Event;
    use crate::model::Thresholds;
    use crate::p2::layout::{PILL_MAX_TEXT_W, PILL_PX, RING_R, RING_W};

    /// 160 minutes of a 51.6° orbit from an hour before `unix`.
    fn track(unix: i64) -> IssTrack {
        let inc = 51.6f64.to_radians();
        let points = (0..481)
            .map(|i| {
                let t = i as f64 * 20.0;
                let u = t / 5_560.0 * std::f64::consts::TAU + 0.6;
                let lat = (inc.sin() * u.sin()).asin();
                let lon =
                    (inc.cos() * u.sin()).atan2(u.cos()) - t / 86_164.0 * std::f64::consts::TAU;
                (
                    lat.to_degrees() as f32,
                    ((lon.to_degrees() + 540.0).rem_euclid(360.0) - 180.0) as f32,
                )
            })
            .collect();
        IssTrack {
            t0: unix - 3600,
            step_secs: 20,
            points,
            home: (52.37, 4.89),
        }
    }

    const UNIX: i64 = 1_788_782_400;

    fn pass() -> IssPass {
        IssPass {
            start: UNIX + 12 * 60 + 40,
            end: UNIX + 18 * 60 + 40,
            max_elevation_deg: 22.4,
            visible: true,
            vis: Some((UNIX + 13 * 60, UNIX + 16 * 60)),
        }
    }

    fn model(unix: i64, pass: Option<IssPass>) -> Model {
        let mut m = Model::new(Thresholds::default());
        m.set_unix_now(unix);
        m.apply(Event::IssPass(pass), 0.0);
        m.apply(Event::IssTrack(track(UNIX)), 0.0);
        m
    }

    fn scene(m: &Model, now: Secs) -> Scene {
        iss_scene(m, now, &mut ScreenUi::new())
    }

    fn runs(s: &Scene) -> Vec<(String, Color)> {
        s.items
            .iter()
            .filter_map(|d| match d {
                Drawable::Run { text, color, .. } => Some((text.clone(), *color)),
                _ => None,
            })
            .collect()
    }

    fn fill(s: &Scene) -> Option<(f32, Color)> {
        s.items[s.base_len..].iter().find_map(|d| match d {
            Drawable::Arc {
                sweep_deg, color, ..
            } => Some((*sweep_deg / crate::p2::layout::RING_SWEEP_DEG, *color)),
            _ => None,
        })
    }

    #[test]
    fn the_pill_counts_down_says_overhead_and_rests() {
        let p = pass();
        assert_eq!(
            pill_text(Some(&p), UNIX),
            ("12:40 · max 22°".into(), TEXT),
            "m:ss under an hour"
        );
        assert_eq!(
            pill_text(Some(&p), UNIX - 3600).0,
            "1:12:40 · max 22°",
            "h:mm:ss from an hour up"
        );
        assert_eq!(pill_text(Some(&p), p.start - 5).0, "0:05 · max 22°");
        assert_eq!(
            pill_text(Some(&p), p.start),
            ("Overhead · max 22°".into(), CYAN)
        );
        assert_eq!(
            pill_text(Some(&p), p.end),
            ("No pass in 24 h".into(), TEXT2),
            "over: until the source finds the next one"
        );
        assert_eq!(pill_text(None, UNIX), ("No pass in 24 h".into(), TEXT2));
        // the scene carries it, at y 190 over an opaque black pill
        let s = scene(&model(UNIX, Some(p)), 0.0);
        assert_eq!(runs(&s), vec![("12:40 · max 22°".to_string(), TEXT)]);
        let rects: Vec<_> = s
            .items
            .iter()
            .filter_map(|d| match d {
                Drawable::RoundRect {
                    y, h, color, alpha, ..
                } => Some((*y + *h / 2.0, *color, *alpha)),
                _ => None,
            })
            .collect();
        assert_eq!(rects, vec![(190.0, BLACK, 1.0), (190.0, TEXT, 0.30)]);
    }

    #[test]
    fn the_longest_pill_fits_its_slot() {
        use crate::p2::font::{measure, FontId};
        let far = IssPass {
            start: UNIX + 23 * 3600 + 59 * 60 + 59,
            max_elevation_deg: 88.0,
            ..pass()
        };
        for text in [
            pill_text(Some(&far), UNIX).0,
            pill_text(Some(&far), far.start).0,
            pill_text(None, 0).0,
        ] {
            let tw = measure(FontId::SemiBold, PILL_PX, &text, 0.0);
            assert!(tw <= PILL_MAX_TEXT_W, "{text:?} is {tw:.1} px");
        }
    }

    #[test]
    fn the_ring_fills_white_over_the_last_ninety_minutes() {
        let p = pass();
        let until = 12.0 * 60.0 + 40.0;
        let s = scene(&model(UNIX, Some(p)), 0.0);
        let (frac, color) = fill(&s).unwrap();
        assert!((frac - (1.0 - until / 5400.0)).abs() < 1e-4, "{frac}");
        assert_eq!(color, TEXT);
        // the standard ring, and the role icon in its gap
        assert!(s.items[..s.base_len].iter().any(
            |d| matches!(d, Drawable::Arc { r, width, .. } if *r == RING_R && *width == RING_W)
        ));
        assert!(s.items[..s.base_len].iter().any(
            |d| matches!(d, Drawable::Icon { name, cy, .. } if *name == "ms-satellite_alt" && *cy == 14.0)
        ));
        assert!(matches!(
            ring(Some(&p), p.start - 2 * 5400).kind,
            RingKind::Empty
        ));
        assert!(
            matches!(ring(Some(&p), p.start + 1).kind, RingKind::Fill { frac, .. } if frac == 1.0)
        );
        assert!(matches!(ring(None, UNIX).kind, RingKind::Empty));
        assert!(fill(&scene(&model(UNIX, None), 0.0)).is_none());
    }

    #[test]
    fn a_second_of_ring_is_shown_at_once_and_a_new_pass_glides() {
        let p = pass();
        let mut ui = ScreenUi::new();
        let mut m = model(UNIX, Some(p));
        iss_scene(&m, 0.0, &mut ui);
        m.set_unix_now(UNIX + 1);
        let a = fill(&iss_scene(&m, 1.0, &mut ui)).unwrap().0;
        let b = fill(&iss_scene(&m, 1.3, &mut ui)).unwrap().0;
        assert_eq!(
            a, b,
            "no glide for a creep: the screen is still between seconds"
        );
        assert!((a - (1.0 - (760.0 - 1.0) / 5400.0)).abs() < 1e-5);
        // a later pass: the fill drops, and that glides
        m.apply(
            Event::IssPass(Some(IssPass {
                start: p.start + 3000,
                end: p.end + 3000,
                ..p
            })),
            0.0,
        );
        let c = fill(&iss_scene(&m, 2.0, &mut ui)).unwrap().0;
        let d = fill(&iss_scene(&m, 2.2, &mut ui)).unwrap().0;
        let e = fill(&iss_scene(&m, 3.0, &mut ui)).unwrap().0;
        assert!(c > d && d > e, "{c} {d} {e}");
        assert!((e - (1.0 - 3759.0 / 5400.0)).abs() < 1e-5);
    }

    fn globe(s: &Scene) -> Option<&Drawable> {
        s.items.iter().find(|d| matches!(d, Drawable::Globe { .. }))
    }

    #[test]
    fn the_globe_follows_the_station_and_is_part_of_the_base() {
        let m = model(UNIX, Some(pass()));
        let s = scene(&m, 0.0);
        let iss = m.iss().track.as_ref().unwrap().at(UNIX).unwrap();
        let (lat, lon) = crate::globe::lat_lon(iss);
        let (sun_lat, sun_lon) = subsolar(UNIX);
        assert!(matches!(s.items[0], Drawable::Clear(_)));
        assert_eq!(
            s.items[1],
            Drawable::Globe {
                cam_lat: (lat * 0.55) as f32,
                cam_lon: (lon - 18.0) as f32,
                sun_lat: sun_lat as f32,
                sun_lon: sun_lon as f32,
                r: 95.0,
            }
        );
        assert!(s.base_len > 1, "the globe's raster is cached with the base");
        // the station: an outlined white icon right of centre
        let icons: Vec<_> = s.items[s.base_len..]
            .iter()
            .filter_map(|d| match d {
                Drawable::IconOutline {
                    cx,
                    cy,
                    width,
                    alpha,
                    ..
                } => Some((*cx, *cy, *width, *alpha, INK)),
                Drawable::Icon {
                    cx,
                    cy,
                    size,
                    alpha,
                    color,
                    ..
                } => Some((*cx, *cy, *size, *alpha, *color)),
                _ => None,
            })
            .collect();
        assert_eq!(icons.len(), 2);
        assert_eq!((icons[0].0, icons[0].1), (icons[1].0, icons[1].1));
        assert_eq!((icons[0].2, icons[0].3), (1.5, 0.85));
        assert_eq!((icons[1].2, icons[1].3, icons[1].4), (20.0, 1.0, WHITE));
        let dx = icons[1].0 - CX;
        assert!(
            (dx - 95.0 * 18f32.to_radians().sin() * (lat as f32).to_radians().cos()).abs() < 2.0,
            "18° east of the centre: {dx}"
        );
    }

    #[test]
    fn the_screen_changes_once_a_second_at_most() {
        let mut m = model(UNIX, Some(pass()));
        let mut ui = ScreenUi::new();
        let a = iss_scene(&m, 10.0, &mut ui);
        let b = iss_scene(&m, 10.9, &mut ui);
        assert_eq!(a, b, "same second, same scene: the frame is skipped");
        m.set_unix_now(UNIX + 1);
        let c = iss_scene(&m, 11.0, &mut ui);
        assert_ne!(globe(&a), globe(&c), "the next second moves the globe");
        // a fresh screen state builds the same thing
        assert_eq!(c, iss_scene(&m, 11.0, &mut ScreenUi::new()));
    }

    #[test]
    fn the_track_is_solid_behind_dashed_ahead_and_stays_on_the_globe() {
        let s = scene(&model(UNIX, Some(pass())), 0.0);
        let lines: Vec<_> = s
            .items
            .iter()
            .filter_map(|d| match d {
                Drawable::Polyline {
                    points,
                    width,
                    color,
                    alpha,
                } => Some((points, *width, *color, *alpha, false)),
                Drawable::Dashed {
                    points,
                    width,
                    on,
                    off,
                    color,
                    alpha,
                } => {
                    assert_eq!((*on, *off), (3.0, 4.0));
                    Some((points, *width, *color, *alpha, true))
                }
                _ => None,
            })
            .collect();
        assert_eq!(lines.len(), 3);
        assert_eq!(
            (lines[0].1, lines[0].2, lines[0].3, lines[0].4),
            (1.5, WHITE, 0.7, true),
            "ahead"
        );
        assert_eq!(
            (lines[1].1, lines[1].2, lines[1].3, lines[1].4),
            (4.0, BLACK, 0.4, false),
            "the shadow"
        );
        assert_eq!(
            (lines[2].1, lines[2].2, lines[2].3, lines[2].4),
            (2.0, WHITE, 1.0, false),
            "behind"
        );
        assert_eq!(lines[1].0, lines[2].0);
        for (points, ..) in &lines {
            assert!(points.len() > 5);
            assert_eq!(points[0], lines[0].0[0], "all start at the station");
            for (x, y) in points.iter() {
                let d = (x - CX).hypot(y - CY);
                assert!(d <= GLOBE_R + 1e-3, "({x}, {y}) is {d} px out");
            }
            let (x, y) = points[points.len() - 1];
            assert!(
                ((x - CX).hypot(y - CY) - GLOBE_R).abs() < 1e-2,
                "the far end is cut at the limb"
            );
        }
        // the station moves west to east: ahead leads right, behind trails left
        assert!(lines[0].0[3].0 > lines[0].0[0].0);
        assert!(lines[2].0[3].0 < lines[2].0[0].0);
    }

    fn dots(s: &Scene) -> Vec<(f32, Color, f32)> {
        s.items
            .iter()
            .filter_map(|d| match d {
                Drawable::Ellipse {
                    rx, color, alpha, ..
                } => Some((*rx, *color, *alpha)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn the_home_dot_shows_only_inside_the_visible_window() {
        let p = pass();
        let (a, b) = p.vis.unwrap();
        // home must be on the near side: put it under the station
        let with_home = |unix: i64| {
            let mut m = model(unix, Some(p));
            let mut tr = track(UNIX);
            let (lat, lon) = crate::globe::lat_lon(tr.at(unix).unwrap());
            tr.home = (lat as f32 - 5.0, lon as f32 - 5.0);
            m.apply(Event::IssTrack(tr), 0.0);
            m
        };
        assert!(dots(&scene(&with_home(a - 1), 0.0)).is_empty(), "before");
        assert!(
            dots(&scene(&with_home(p.start + 1), 0.0)).is_empty(),
            "the pass has begun, but not yet where it can be seen"
        );
        let on = scene(&with_home(a), 0.0);
        assert_eq!(dots(&on), vec![(3.6, INK, 0.9), (2.6, WHITE, 1.0)]);
        // drawn above the station
        let icon = on
            .items
            .iter()
            .rposition(|d| matches!(d, Drawable::Icon { .. }))
            .unwrap();
        let dot = on
            .items
            .iter()
            .position(|d| matches!(d, Drawable::Ellipse { .. }))
            .unwrap();
        assert!(dot > icon);
        assert_eq!(dots(&scene(&with_home(b - 1), 0.0)).len(), 2);
        assert!(dots(&scene(&with_home(b), 0.0)).is_empty(), "after");
        // a pass that cannot be seen never shows it
        let mut dark = with_home(a + 10);
        dark.apply(Event::IssPass(Some(IssPass { vis: None, ..p })), 0.0);
        assert!(dots(&scene(&dark, 0.0)).is_empty());
        // it fades in and out
        let mut ui = ScreenUi::new();
        iss_scene(&with_home(a - 1), 0.0, &mut ui);
        let half = dots(&iss_scene(&with_home(a), 1.0, &mut ui));
        let mid = dots(&iss_scene(&with_home(a), 1.1, &mut ui));
        assert!(half.is_empty(), "the glide starts from nothing");
        assert!(mid[1].2 > 0.05 && mid[1].2 < 1.0, "{mid:?}");
        assert_eq!(dots(&iss_scene(&with_home(a), 2.0, &mut ui))[1].2, 1.0);
        // home on the far side of the globe: no dot
        let mut far = model(a, Some(p));
        let mut tr = track(UNIX);
        let (lat, lon) = crate::globe::lat_lon(tr.at(a).unwrap());
        tr.home = (-lat as f32, lon as f32 + 180.0);
        far.apply(Event::IssTrack(tr), 0.0);
        assert!(dots(&scene(&far, 0.0)).is_empty());
    }

    #[test]
    fn without_an_ephemeris_the_ring_and_pill_still_draw() {
        let mut m = Model::new(Thresholds::default());
        m.set_unix_now(UNIX);
        m.apply(Event::IssPass(Some(pass())), 0.0);
        let s = scene(&m, 0.0);
        assert!(globe(&s).is_none());
        assert_eq!(runs(&s).len(), 1);
        // an ephemeris that does not reach now draws no globe either
        let mut old = model(UNIX + 3 * 3600, None);
        assert!(globe(&scene(&old, 0.0)).is_none());
        old.set_unix_now(UNIX + 100 * 60);
        assert!(
            globe(&scene(&old, 0.0)).is_some(),
            "its last point still does"
        );
    }
}
