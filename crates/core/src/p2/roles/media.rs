//! The media library and download card, and the Jellyfin now-playing tile.

use crate::anim::Secs;
use crate::event::Download;
use crate::format::{fmt_ago, fmt_left};
use crate::model::Model;
use crate::p2::chart::Card;
use crate::p2::ring::RingSpec;
use crate::p2::screen::P2Screen;
use crate::theme::p2::{BLUE, CYAN, ORANGE, TEXT2, VIOLET};

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
}
