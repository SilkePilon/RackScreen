//! The media library and download card, and the Jellyfin now-playing tile.

use crate::anim::Secs;
use crate::event::{Download, JfKind, JfSession, JfStatus};
use crate::format::{fmt_ago, fmt_hm, fmt_hms, fmt_left};
use crate::model::Model;
use crate::p2::chart::{Card, CardLine};
use crate::p2::font::FontId;
use crate::p2::ring::RingSpec;
use crate::p2::screen::P2Screen;
use crate::theme::p2::{BLUE, CYAN, OK, ORANGE, TEXT, TEXT2, TEXT3, VIOLET, WARN};

/// "added just now" shows for this long after an item leaves the queue.
pub const ADDED_JUST_NOW_SECS: Secs = 60.0;

/// The download to show and how many are running: the arr pick, or without
/// arr keys the most complete qBittorrent torrent.
fn current_download(model: &Model) -> Option<(Download, usize)> {
    let m = model.media();
    if let Some(d) = &m.download {
        return Some((d.clone(), 1));
    }
    if model.arr_configured() || !model.link().qbit {
        return None;
    }
    let list = model.torrent_badge_list();
    let top = list
        .iter()
        .max_by(|a, b| a.progress.total_cmp(&b.progress))?;
    Some((
        Download {
            title: top.name.clone(),
            progress: top.progress / 100.0,
            eta_secs: top.eta_secs,
        },
        list.len(),
    ))
}

pub fn media(model: &Model, now: Secs) -> P2Screen {
    let m = model.media();
    let total = m.movies + m.series + m.artists;
    let denom = total.max(1) as f32;
    let mut s = P2Screen::new("MEDIA", total.to_string());
    s.ring = RingSpec::parts(vec![
        (m.movies as f32 / denom, ORANGE),
        (m.series as f32 / denom, CYAN),
        (m.artists as f32 / denom, VIOLET),
    ]);
    let download = current_download(model);
    s.pill = Some(match &download {
        Some((_, n)) => (format!("{n} downloading"), BLUE),
        None => (format!("library {total}"), TEXT2),
    });
    s.card = Some(match (download, &m.last_added) {
        (Some((d, _)), _) => Card::Download {
            label: format!("{:.0} % · {}", d.progress * 100.0, fmt_left(d.eta_secs)),
            title: d.title,
            progress: d.progress,
            done: false,
        },
        (None, Some((title, at))) => {
            let age = now - at;
            Card::Download {
                title: title.clone(),
                progress: 1.0,
                label: if age < ADDED_JUST_NOW_SECS {
                    "added just now".into()
                } else {
                    format!("added {}", fmt_ago(age))
                },
                done: true,
            }
        }
        (None, None) => Card::Stats(vec![
            (m.movies.to_string(), "movies".into(), ORANGE),
            (m.series.to_string(), "series".into(), CYAN),
            (m.artists.to_string(), "artists".into(), VIOLET),
        ]),
    });
    s
}

/// The session to show: of the playing ones the one that started most
/// recently (the smallest position); with everything paused, the most
/// recently active one.
pub fn pick_session(sessions: &[JfSession]) -> Option<&JfSession> {
    sessions
        .iter()
        .filter(|s| !s.paused)
        .min_by(|a, b| a.position_secs.total_cmp(&b.position_secs))
        .or_else(|| {
            sessions
                .iter()
                .max_by(|a, b| a.last_activity.cmp(&b.last_activity))
        })
}

/// Playback position now: the polled position plus the time since the poll
/// while playing, never past the end.
pub fn position(s: &JfSession, polled_at: Secs, now: Secs) -> f64 {
    let elapsed = if s.paused {
        0.0
    } else {
        (now - polled_at).max(0.0)
    };
    (s.position_secs + elapsed).min(s.runtime_secs.max(0.0))
}

/// The title line: the series for an episode, the item name otherwise.
pub fn display_title(s: &JfSession) -> String {
    match (&s.kind, &s.series) {
        (JfKind::Episode, Some(series)) => series.clone(),
        _ => s.title.clone(),
    }
}

fn subtitle(s: &JfSession) -> String {
    match s.kind {
        JfKind::Episode => {
            let mut parts = Vec::new();
            if let Some(n) = s.season {
                parts.push(format!("S{n}"));
            }
            if let Some(n) = s.episode {
                parts.push(format!("E{n}"));
            }
            parts.push(s.title.clone());
            parts.join(" · ")
        }
        _ => match s.year {
            Some(y) => format!("{y} · {}", fmt_hm(s.runtime_secs as i64)),
            None => fmt_hm(s.runtime_secs as i64),
        },
    }
}

fn empty_tile(pill: (String, crate::theme::Color)) -> P2Screen {
    let mut s = P2Screen::new("NOW PLAYING", "—");
    s.hero_color = TEXT3;
    s.pill = Some(pill);
    s
}

pub fn jellyfin(model: &Model, now: Secs) -> P2Screen {
    let j = model.jellyfin();
    match j.status {
        JfStatus::Offline => return empty_tile(("Jellyfin offline".into(), WARN)),
        JfStatus::Unauthorized => return empty_tile(("Check API key".into(), WARN)),
        JfStatus::Ok => {}
    }
    let Some(s) = pick_session(&j.sessions) else {
        let mut t = empty_tile(("Nothing playing".into(), TEXT2));
        if let Some((title, at)) = &j.last_seen {
            let ago = fmt_ago((model.unix_now() - at) as f64);
            t.card = Some(Card::Lines(vec![
                CardLine {
                    text: "last watched".into(),
                    font: FontId::Medium,
                    px: 8.5,
                    color: TEXT3,
                    dy: 4.0,
                },
                CardLine {
                    text: format!("{title} · {ago}"),
                    font: FontId::SemiBold,
                    px: 10.0,
                    color: TEXT2,
                    dy: 18.0,
                },
            ]));
        }
        return t;
    };
    let pos = position(s, j.polled_at, now);
    let left = (s.runtime_secs - pos).max(0.0);
    let mut t = P2Screen::new("NOW PLAYING", fmt_hms(left));
    t.hero_scale = if left >= 3600.0 { 0.62 } else { 0.82 };
    t.unit = Some("left".into());
    t.unit_below = true;
    if s.paused {
        t.hero_color = TEXT2;
    }
    let color = if s.kind == JfKind::Movie {
        ORANGE
    } else {
        CYAN
    };
    t.ring = RingSpec::fill(
        (pos / s.runtime_secs.max(1.0)) as f32,
        if s.paused {
            color.with_alpha(0.45)
        } else {
            color
        },
    );
    t.pill = Some(if s.paused {
        ("Paused".into(), WARN)
    } else if s.transcoding {
        ("Transcoding".into(), WARN)
    } else {
        ("Direct play".into(), OK)
    });
    let others = j.sessions.len().saturating_sub(1);
    let mut foot = format!("{} · {}", s.user, s.device);
    if others > 0 {
        foot.push_str(&format!("  ·  +{others} more"));
    }
    t.card = Some(Card::Lines(vec![
        CardLine {
            text: display_title(s),
            font: FontId::Bold,
            px: 11.0,
            color: TEXT,
            dy: 7.0,
        },
        CardLine {
            text: subtitle(s),
            font: FontId::Medium,
            px: 8.5,
            color: TEXT2,
            dy: 21.0,
        },
        CardLine {
            text: foot,
            font: FontId::Medium,
            px: 8.5,
            color: TEXT3,
            dy: 32.0,
        },
    ]));
    t
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{Download, Event, LinkTarget, Torrent};
    use crate::model::Thresholds;
    use crate::p2::chart::Card;
    use crate::p2::ring::RingKind;
    use crate::theme::p2::{BLUE, CYAN, OK, ORANGE, TEXT2, VIOLET};

    fn library() -> Model {
        let mut m = Model::new(Thresholds::default());
        m.apply(
            Event::MediaLibrary {
                movies: 37,
                series: 30,
                artists: 54,
            },
            0.0,
        );
        m
    }

    #[test]
    fn media_splits_the_library_and_shows_the_download() {
        let mut m = library();
        let s = media(&m, 0.0);
        assert_eq!((s.title.as_str(), s.hero.as_str()), ("MEDIA", "121"));
        match &s.ring.kind {
            RingKind::Parts(p) => {
                assert!((p[0].0 - 37.0 / 121.0).abs() < 1e-5 && p[0].1 == ORANGE);
                assert!((p[1].0 - 30.0 / 121.0).abs() < 1e-5 && p[1].1 == CYAN);
                assert!((p[2].0 - 54.0 / 121.0).abs() < 1e-5 && p[2].1 == VIOLET);
            }
            _ => panic!("parts"),
        }
        assert_eq!(s.pill, Some(("library 121".into(), TEXT2)));
        assert!(
            matches!(&s.card, Some(Card::Stats(items)) if items.len() == 3 && items[0] == ("37".to_string(), "movies".to_string(), ORANGE) && items[2].1 == "artists"),
            "nothing queued, nothing added: the counts"
        );
        m.apply(
            Event::Download(Some(Download {
                title: "Dune: Part Two".into(),
                progress: 0.35,
                eta_secs: 19 * 60 + 12,
            })),
            1.0,
        );
        let s = media(&m, 1.0);
        assert_eq!(s.pill, Some(("1 downloading".into(), BLUE)));
        assert_eq!(
            s.card,
            Some(Card::Download {
                title: "Dune: Part Two".into(),
                progress: 0.35,
                label: "35 % · 19 min left".into(),
                done: false,
            })
        );
        m.apply(Event::Download(None), 2.0);
        m.apply(
            Event::MediaAdded {
                title: "Dune: Part Two".into(),
            },
            2.0,
        );
        let s = media(&m, 30.0);
        assert_eq!(s.pill, Some(("library 121".into(), TEXT2)));
        assert_eq!(
            s.card,
            Some(Card::Download {
                title: "Dune: Part Two".into(),
                progress: 1.0,
                label: "added just now".into(),
                done: true,
            })
        );
        let s = media(&m, 2.0 + 2.0 * 3600.0);
        assert!(
            matches!(&s.card, Some(Card::Download { label, done: true, .. }) if label == "added 2 h ago")
        );
    }

    #[test]
    fn media_falls_back_to_the_torrent_without_arr_keys() {
        let mut m = library();
        m.apply(
            Event::Link {
                target: LinkTarget::QBittorrent,
                up: true,
            },
            0.0,
        );
        m.apply(
            Event::Torrents(vec![
                Torrent {
                    name: "ubuntu-24.04.iso".into(),
                    progress: 78.0,
                    eta_secs: 900,
                    speed_bps: 9_000_000,
                },
                Torrent {
                    name: "debian-13.iso".into(),
                    progress: 41.0,
                    eta_secs: 3000,
                    speed_bps: 3_000_000,
                },
            ]),
            0.0,
        );
        let s = media(&m, 0.0);
        assert_eq!(s.pill, Some(("2 downloading".into(), BLUE)));
        assert!(
            matches!(&s.card, Some(Card::Download { title, progress, label, done: false }) if title == "ubuntu-24.04.iso" && (*progress - 0.78).abs() < 1e-6 && label == "78 % · 15 min left")
        );
        m.set_arr_configured(true);
        let s = media(&m, 1.0);
        assert_eq!(
            s.pill,
            Some(("library 121".into(), TEXT2)),
            "with arr keys the torrent is not the media card"
        );
        assert!(matches!(&s.card, Some(Card::Stats(_))));
        assert_eq!(OK, crate::theme::p2::OK);
    }
    use crate::event::{JfKind, JfSession, JfStatus};
    use crate::p2::chart::CardLine;
    use crate::p2::font::FontId;
    use crate::theme::p2::{TEXT, TEXT3, WARN};

    fn movie(pos: f64, paused: bool) -> JfSession {
        JfSession {
            title: "Blade Runner 2049".into(),
            kind: JfKind::Movie,
            series: None,
            season: None,
            episode: None,
            year: Some(2017),
            runtime_secs: 9840.0,
            position_secs: pos,
            paused,
            transcoding: false,
            user: "silke".into(),
            device: "Living room TV".into(),
            last_activity: "2026-09-07T12:00:05.000Z".into(),
        }
    }

    fn episode() -> JfSession {
        JfSession {
            title: "Home".into(),
            kind: JfKind::Episode,
            series: Some("The Expanse".into()),
            season: Some(2),
            episode: Some(5),
            year: None,
            runtime_secs: 2640.0,
            position_secs: 1480.0,
            paused: false,
            transcoding: true,
            user: "sam".into(),
            device: "iPad".into(),
            last_activity: "2026-09-07T12:00:02.000Z".into(),
        }
    }

    fn jf(status: JfStatus, sessions: Vec<JfSession>, polled_at: f64) -> Model {
        let mut m = Model::new(Thresholds::default());
        m.set_unix_now(1_788_782_400);
        m.apply(Event::Jellyfin { status, sessions }, polled_at);
        m
    }

    #[test]
    fn session_pick_and_position_extrapolation() {
        let playing_late = movie(3720.0, false);
        let mut playing_fresh = episode();
        playing_fresh.position_secs = 10.0;
        let paused = movie(6000.0, true);
        assert_eq!(
            pick_session(&[paused.clone(), playing_late.clone(), playing_fresh.clone()])
                .unwrap()
                .title,
            "Home",
            "the playing session that started most recently"
        );
        let mut paused_old = paused.clone();
        paused_old.last_activity = "2026-09-07T11:00:00.000Z".into();
        assert_eq!(
            pick_session(&[paused_old, paused.clone()])
                .unwrap()
                .position_secs,
            6000.0,
            "all paused: the most recently active"
        );
        assert!(pick_session(&[]).is_none());
        assert!((position(&playing_late, 10.0, 25.5) - 3735.5).abs() < 1e-6);
        assert!(
            (position(&paused, 10.0, 25.5) - 6000.0).abs() < 1e-6,
            "paused does not move"
        );
        assert!(
            (position(&playing_late, 10.0, 5.0) - 3720.0).abs() < 1e-6,
            "never backwards"
        );
        let mut near_end = playing_late.clone();
        near_end.position_secs = 9835.0;
        assert!(
            (position(&near_end, 0.0, 100.0) - 9840.0).abs() < 1e-6,
            "clamped to the runtime"
        );
        assert_eq!(display_title(&episode()), "The Expanse");
        assert_eq!(display_title(&movie(0.0, false)), "Blade Runner 2049");
    }

    #[test]
    fn jellyfin_tile_for_a_movie_an_episode_pause_idle_and_errors() {
        let m = jf(JfStatus::Ok, vec![movie(3720.0, false), episode()], 10.0);
        let s = jellyfin(&m, 16.0);
        // the episode started most recently (position 1480 < 3720): 2640 - (1480 + 6) = 1154 s
        assert_eq!(
            (
                s.title.as_str(),
                s.hero.as_str(),
                s.hero_scale,
                s.unit.as_deref(),
                s.unit_below
            ),
            ("NOW PLAYING", "19:14", 0.82, Some("left"), true)
        );
        assert!(
            matches!(s.ring.kind, RingKind::Fill { frac, color } if (frac - 1486.0 / 2640.0).abs() < 1e-5 && color == CYAN)
        );
        assert_eq!(s.pill, Some(("Transcoding".into(), WARN)));
        assert_eq!(
            s.card,
            Some(Card::Lines(vec![
                CardLine {
                    text: "The Expanse".into(),
                    font: FontId::Bold,
                    px: 11.0,
                    color: TEXT,
                    dy: 7.0
                },
                CardLine {
                    text: "S2 · E5 · Home".into(),
                    font: FontId::Medium,
                    px: 8.5,
                    color: TEXT2,
                    dy: 21.0
                },
                CardLine {
                    text: "sam · iPad  ·  +1 more".into(),
                    font: FontId::Medium,
                    px: 8.5,
                    color: TEXT3,
                    dy: 32.0
                },
            ]))
        );
        let m = jf(JfStatus::Ok, vec![movie(3720.0, false)], 10.0);
        let s = jellyfin(&m, 16.0);
        assert_eq!((s.hero.as_str(), s.hero_scale), ("1:41:54", 0.62));
        assert!(
            matches!(s.ring.kind, RingKind::Fill { frac, color } if (frac - 3726.0 / 9840.0).abs() < 1e-5 && color == ORANGE)
        );
        assert_eq!(s.pill, Some(("Direct play".into(), OK)));
        assert!(
            matches!(&s.card, Some(Card::Lines(l)) if l[0].text == "Blade Runner 2049" && l[1].text == "2017 · 2h 44m" && l[2].text == "silke · Living room TV")
        );
        let m = jf(JfStatus::Ok, vec![movie(3720.0, true)], 10.0);
        let s = jellyfin(&m, 16.0);
        assert_eq!(s.hero, "1:42:00", "paused: no extrapolation");
        assert_eq!(s.hero_color, TEXT2);
        assert!(
            matches!(s.ring.kind, RingKind::Fill { color, .. } if color == ORANGE.with_alpha(0.45))
        );
        assert_eq!(s.pill, Some(("Paused".into(), WARN)));
        let mut idle = jf(JfStatus::Ok, vec![movie(3720.0, false)], 10.0);
        idle.set_unix_now(1_788_782_400 + 7200);
        idle.apply(
            Event::Jellyfin {
                status: JfStatus::Ok,
                sessions: vec![],
            },
            20.0,
        );
        let s = jellyfin(&idle, 21.0);
        assert_eq!((s.hero.as_str(), s.hero_color), ("—", TEXT3));
        assert!(matches!(s.ring.kind, RingKind::Empty));
        assert_eq!(s.pill, Some(("Nothing playing".into(), TEXT2)));
        assert!(
            matches!(&s.card, Some(Card::Lines(l)) if l[0].text == "last watched" && l[1].text == "Blade Runner 2049 · 2 h ago")
        );
        let fresh = jf(JfStatus::Ok, vec![], 1.0);
        assert!(
            jellyfin(&fresh, 2.0).card.is_none(),
            "no card before any item was seen"
        );
        let off = jf(JfStatus::Offline, vec![], 1.0);
        let s = jellyfin(&off, 2.0);
        assert_eq!(
            (s.hero.as_str(), s.pill.clone().unwrap()),
            ("—", ("Jellyfin offline".to_string(), WARN))
        );
        let key = jf(JfStatus::Unauthorized, vec![], 1.0);
        assert_eq!(
            jellyfin(&key, 2.0).pill,
            Some(("Check API key".into(), WARN))
        );
    }
}
