//! The music screen (look N1): album cover up top, track progress on the
//! ring in the source's colour, time left in the pill, and song, artist and
//! device in the card. Spotify and Jellyfin feed it; `music::pick` chooses.

use crate::anim::Secs;
use crate::event::{MusicSource, Track};
use crate::format::fmt_hms;
use crate::model::Model;
use crate::music::{fmt_played_ago, position, COVER_PX};
use crate::p2::chart::{card_drawables, Card, CardLine};
use crate::p2::font::FontId;
use crate::p2::layout::{
    CARD_H, CARD_R, CARD_W, CHART_INSET_BOTTOM, CHART_INSET_TOP, CHART_INSET_X,
};
use crate::p2::ring::RingSpec;
use crate::p2::text::{pill, top_icon};
use crate::p2::ui::ScreenUi;
use crate::scene::{Drawable, Scene};
use crate::theme::layout::CX;
use crate::theme::p2::{CARD, GREEN, TEXT, TEXT2, TEXT3, VIOLET};
use crate::theme::{Color, Role};

/// The cover square: 84 px at (78, 36), corners `CARD_R`.
pub const COVER_X: f32 = 78.0;
pub const COVER_Y: f32 = 36.0;
pub const COVER_SIZE: f32 = COVER_PX as f32;
pub const PILL_CY: f32 = 136.0;
pub const CARD_Y: f32 = 152.0;
/// Paused ring and idle cover.
pub const DIM_ALPHA: f32 = 0.45;
/// The note on a cover-less track, `TEXT3`.
pub const PLACEHOLDER_ICON_PX: f32 = 36.0;
/// The ring's fill moves in steps of a quarter degree, so a playing track
/// redraws a few times a second rather than every frame.
pub const RING_STEPS: f32 = 328.0 * 4.0;

pub fn source_color(s: MusicSource) -> Color {
    match s {
        MusicSource::Spotify => GREEN,
        MusicSource::Jellyfin => VIOLET,
    }
}

/// What the screen shows, without the drawing.
#[derive(Clone, Debug, PartialEq)]
pub struct MusicView {
    /// Fraction of the track played and the ring colour (alpha included);
    /// `None` leaves the ring empty.
    pub ring: Option<(f32, Color)>,
    pub pill: (String, Color),
    /// The track whose cover and lines show, and the cover's alpha.
    pub track: Option<(Track, f32)>,
}

pub fn view(model: &Model, now: Secs) -> MusicView {
    let m = model.music();
    if let Some(t) = m.current() {
        let pos = position(t, m.polled_at(t.source), now);
        let frac = (pos / t.duration_secs.max(1.0)) as f32;
        let color = source_color(t.source);
        return if t.playing {
            MusicView {
                ring: Some((frac, color)),
                pill: (format!("{} left", fmt_hms(t.duration_secs - pos)), color),
                track: Some((t.clone(), 1.0)),
            }
        } else {
            MusicView {
                ring: Some((frac, color.with_alpha(DIM_ALPHA))),
                pill: ("paused".into(), TEXT2),
                track: Some((t.clone(), 1.0)),
            }
        };
    }
    match &m.last {
        Some((t, at)) => MusicView {
            ring: None,
            pill: (
                format!("played {}", fmt_played_ago(model.unix_now() - at)),
                TEXT2,
            ),
            track: Some((t.clone(), DIM_ALPHA)),
        },
        None => MusicView {
            ring: None,
            pill: ("nothing playing".into(), TEXT2),
            track: None,
        },
    }
}

fn lines(t: &Track) -> Card {
    Card::Lines(vec![
        CardLine {
            text: t.title.clone(),
            font: FontId::SemiBold,
            px: 11.0,
            color: TEXT,
            dy: 7.0,
        },
        CardLine {
            text: t.artist.clone(),
            font: FontId::Medium,
            px: 8.5,
            color: TEXT2,
            dy: 21.0,
        },
        CardLine {
            text: t.device.clone(),
            font: FontId::Medium,
            px: 8.5,
            color: TEXT2,
            dy: 32.0,
        },
    ])
}

pub fn music_scene(model: &Model, now: Secs, ui: &mut ScreenUi) -> Scene {
    let v = view(model, now);
    let mut scene = Scene::new();
    let ring = match v.ring {
        Some((frac, color)) => {
            let f = ui.glide(("ring", 0), frac.clamp(0.0, 1.0), now);
            RingSpec::fill((f * RING_STEPS).round() / RING_STEPS, color)
        }
        None => RingSpec::empty(),
    };
    // Base: track, icon, cover, card background; none overlaps another or
    // the ring fill, so the base is pixel-neutral as a prefix.
    scene.items.extend(ring.track());
    scene.push(top_icon(Role::Music.icon()));
    if let Some((t, alpha)) = &v.track {
        match &t.cover {
            Some(cover) => scene.push(Drawable::Cover {
                cover: cover.clone(),
                x: COVER_X,
                y: COVER_Y,
                size: COVER_SIZE,
                r: CARD_R,
                alpha: *alpha,
            }),
            None => {
                scene.push(Drawable::RoundRect {
                    x: COVER_X,
                    y: COVER_Y,
                    w: COVER_SIZE,
                    h: COVER_SIZE,
                    r: CARD_R,
                    color: CARD,
                    alpha: 1.0,
                });
                scene.push(Drawable::Icon {
                    name: Role::Music.icon(),
                    cx: COVER_X + COVER_SIZE / 2.0,
                    cy: COVER_Y + COVER_SIZE / 2.0,
                    size: PLACEHOLDER_ICON_PX,
                    color: TEXT3,
                    alpha: 1.0,
                    scale: 1.0,
                    dy: 0.0,
                });
            }
        }
    }
    let card_x = CX - CARD_W / 2.0;
    scene.push(Drawable::RoundRect {
        x: card_x,
        y: CARD_Y,
        w: CARD_W,
        h: CARD_H,
        r: CARD_R,
        color: CARD,
        alpha: 1.0,
    });
    scene.base_len = scene.items.len();
    scene.items.extend(ring.overlay());
    scene
        .items
        .extend(pill(ui, &v.pill.0, v.pill.1, PILL_CY, now));
    if let Some((t, _)) = &v.track {
        let area = (
            card_x + CHART_INSET_X,
            CARD_Y + CHART_INSET_TOP,
            CARD_W - 2.0 * CHART_INSET_X,
            CARD_H - CHART_INSET_TOP - CHART_INSET_BOTTOM,
        );
        scene.items.extend(card_drawables(&lines(t), ui, area, now));
    }
    scene
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{CoverRef, Event};
    use crate::model::Thresholds;
    use crate::music::demo_cover;
    use crate::p2::layout::RING_SWEEP_DEG;
    use crate::theme::p2::TRACK;

    const UNIX: i64 = 1_788_782_400;

    fn tune(source: MusicSource, playing: bool, cover: bool) -> Track {
        Track {
            source,
            title: "Holocene".into(),
            artist: "Bon Iver".into(),
            device: "Kitchen speaker".into(),
            duration_secs: 337.0,
            position_secs: 120.0,
            playing,
            since: UNIX - 120,
            cover: cover.then(|| {
                CoverRef::new(demo_cover(
                    "album",
                    Color::hex(0x335577),
                    Color::hex(0xcc8844),
                ))
            }),
        }
    }

    fn model(tracks: Vec<Track>) -> Model {
        let mut m = Model::new(Thresholds::default());
        m.set_unix_now(UNIX);
        m.set_music_configured(true);
        for t in tracks {
            m.apply(
                Event::Music {
                    source: t.source,
                    track: Some(t),
                },
                10.0,
            );
        }
        m
    }

    fn runs(s: &Scene) -> Vec<String> {
        s.items
            .iter()
            .filter_map(|d| match d {
                Drawable::Run { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn playing_spotify_counts_down_in_green() {
        let m = model(vec![tune(MusicSource::Spotify, true, true)]);
        let v = view(&m, 16.0);
        // 337 - (120 + 6) = 211 s
        assert_eq!(v.pill, ("3:31 left".to_string(), GREEN));
        let (frac, color) = v.ring.unwrap();
        assert!((frac - 126.0 / 337.0).abs() < 1e-5 && color == GREEN);
        assert_eq!(v.track.as_ref().unwrap().1, 1.0);
    }

    #[test]
    fn jellyfin_is_violet_and_paused_is_dim() {
        let m = model(vec![tune(MusicSource::Jellyfin, true, true)]);
        assert_eq!(view(&m, 10.0).pill.1, VIOLET);
        let m = model(vec![tune(MusicSource::Jellyfin, false, true)]);
        let v = view(&m, 99.0);
        assert_eq!(v.pill, ("paused".to_string(), TEXT2));
        let (frac, color) = v.ring.unwrap();
        assert!(
            (frac - 120.0 / 337.0).abs() < 1e-5,
            "no ticking while paused"
        );
        assert_eq!(color, VIOLET.with_alpha(DIM_ALPHA));
    }

    #[test]
    fn idle_shows_the_last_track_dimmed_and_when() {
        let mut m = model(vec![tune(MusicSource::Spotify, true, true)]);
        m.set_unix_now(UNIX + 12 * 60);
        m.apply(
            Event::Music {
                source: MusicSource::Spotify,
                track: None,
            },
            20.0,
        );
        let v = view(&m, 21.0);
        assert_eq!(v.pill, ("played 12 min ago".to_string(), TEXT2));
        assert!(v.ring.is_none());
        assert_eq!(
            v.track.as_ref().map(|(t, a)| (t.title.as_str(), *a)),
            Some(("Holocene", DIM_ALPHA))
        );
        let s = music_scene(&m, 21.0, &mut ScreenUi::new());
        assert!(s
            .items
            .iter()
            .any(|d| matches!(d, Drawable::Cover { alpha, .. } if *alpha == DIM_ALPHA)));
    }

    #[test]
    fn nothing_ever_played_has_no_cover_and_an_empty_card() {
        let mut m = model(vec![]);
        m.apply(
            Event::Music {
                source: MusicSource::Jellyfin,
                track: None,
            },
            1.0,
        );
        let s = music_scene(&m, 2.0, &mut ScreenUi::new());
        assert_eq!(runs(&s), vec!["nothing playing".to_string()]);
        assert!(!s.items.iter().any(|d| matches!(d, Drawable::Cover { .. })));
        assert_eq!(
            s.items
                .iter()
                .filter(|d| matches!(d, Drawable::RoundRect { color, .. } if *color == CARD))
                .count(),
            1,
            "the empty card"
        );
    }

    #[test]
    fn scene_base_is_track_icon_cover_card_then_fill_pill_lines() {
        let m = model(vec![tune(MusicSource::Spotify, true, true)]);
        let s = music_scene(&m, 10.0, &mut ScreenUi::new());
        let it = &s.items;
        assert!(matches!(it[0], Drawable::Clear(_)));
        assert!(matches!(it[1], Drawable::Arc { color, .. } if color == TRACK));
        assert!(matches!(it[2], Drawable::Icon { name, .. } if name == "ms-music_note"));
        assert!(
            matches!(&it[3], Drawable::Cover { x, y, size, r, alpha, .. }
            if *x == COVER_X && *y == COVER_Y && *size == 84.0 && *r == CARD_R && *alpha == 1.0)
        );
        assert!(
            matches!(it[4], Drawable::RoundRect { y, color, .. } if y == CARD_Y && color == CARD)
        );
        assert_eq!(s.base_len, 5);
        assert!(matches!(it[5], Drawable::Arc { color, sweep_deg, .. }
            if color == GREEN && (sweep_deg - RING_SWEEP_DEG * 120.0 / 337.0).abs() < 0.5));
        assert_eq!(
            runs(&s),
            vec!["3:37 left", "Holocene", "Bon Iver", "Kitchen speaker"]
        );
    }

    #[test]
    fn a_track_without_art_gets_a_placeholder_note() {
        let m = model(vec![tune(MusicSource::Spotify, true, false)]);
        let s = music_scene(&m, 10.0, &mut ScreenUi::new());
        assert!(matches!(s.items[3], Drawable::RoundRect { x, w, color, .. }
            if x == COVER_X && w == COVER_SIZE && color == CARD));
        assert!(matches!(s.items[4], Drawable::Icon { name, color, .. }
            if name == "ms-music_note" && color == TEXT3));
        assert_eq!(s.base_len, 6);
    }

    #[test]
    fn the_ring_moves_in_quarter_degree_steps() {
        let m = model(vec![tune(MusicSource::Spotify, true, true)]);
        let mut ui = ScreenUi::new();
        music_scene(&m, 10.0, &mut ui);
        let a = music_scene(&m, 11.0, &mut ui);
        let b = music_scene(&m, 11.01, &mut ui);
        assert_eq!(a.items[5], b.items[5], "10 ms does not move the ring");
    }

    #[test]
    fn cover_and_card_stay_inside_the_ring() {
        use crate::p2::layout::{RING_R, RING_W};
        use crate::theme::layout::CY;
        let inner = RING_R - RING_W / 2.0;
        for (x, y) in [
            (COVER_X, COVER_Y),
            (COVER_X + COVER_SIZE, COVER_Y),
            (CX - CARD_W / 2.0, CARD_Y + CARD_H),
            (CX + CARD_W / 2.0, CARD_Y + CARD_H),
        ] {
            assert!((x - CX).hypot(y - CY) < inner, "({x}, {y})");
        }
        const { assert!(PILL_CY + 10.0 < CARD_Y && COVER_Y + COVER_SIZE < PILL_CY - 10.0) };
    }
}
