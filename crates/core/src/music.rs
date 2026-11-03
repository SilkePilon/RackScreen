//! Music rules shared by the sources and the music screen: which source
//! shows, where playback is now, and when a track last changed state.

use crate::anim::Secs;
use crate::event::{Cover, Track};
use crate::theme::Color;

/// Side of the cover on the screen, and of the squared art the sources keep.
pub const COVER_PX: u32 = 84;

/// The track to show of two sources: a playing one beats a paused one;
/// between two of the same kind the later `since` wins.
pub fn pick<'a>(a: Option<&'a Track>, b: Option<&'a Track>) -> Option<&'a Track> {
    match (a, b) {
        (Some(x), Some(y)) => {
            if (y.playing, y.since) > (x.playing, x.since) {
                Some(y)
            } else {
                Some(x)
            }
        }
        (x, None) => x,
        (None, y) => y,
    }
}

/// Playback position now: the polled position plus the time since the poll
/// while playing, never backwards and never past the end.
pub fn position(t: &Track, polled_at: Secs, now: Secs) -> f64 {
    let elapsed = if t.playing {
        (now - polled_at).max(0.0)
    } else {
        0.0
    };
    (t.position_secs + elapsed).min(t.duration_secs.max(0.0))
}

/// Carry `since` over from the previous poll when the same track is in the
/// same play state; otherwise it changed now.
pub fn stamp_since(prev: Option<&Track>, next: &mut Track, unix_now: i64) {
    next.since = match prev {
        Some(p)
            if p.title == next.title && p.artist == next.artist && p.playing == next.playing =>
        {
            p.since
        }
        _ => unix_now,
    };
}

/// `just now`, `12 min ago`, `2 h ago`, `3 d ago`.
pub fn fmt_played_ago(secs: i64) -> String {
    match secs.max(0) {
        s if s < 60 => "just now".into(),
        s if s < 3600 => format!("{} min ago", s / 60),
        s if s < 86_400 => format!("{} h ago", s / 3600),
        s => format!("{} d ago", s / 86_400),
    }
}

/// A made-up album cover (the fake source and the goldens): a diagonal
/// gradient from `a` to `b` with a soft disc, `COVER_PX` square.
pub fn demo_cover(key: &str, a: Color, b: Color) -> Cover {
    let n = COVER_PX;
    let mut rgba = Vec::with_capacity((n * n * 4) as usize);
    let c = n as f32 / 2.0;
    for y in 0..n {
        for x in 0..n {
            let t = (x + y) as f32 / (2 * (n - 1)) as f32;
            let mut px = a.mix(b, t);
            let d = ((x as f32 + 0.5 - c * 1.2).hypot(y as f32 + 0.5 - c * 0.8)) / c;
            if d < 0.55 {
                px = px.mix(crate::theme::WHITE, 0.25 * (1.0 - d / 0.55));
            }
            rgba.extend_from_slice(&[px.r, px.g, px.b, 255]);
        }
    }
    Cover {
        key: key.to_string(),
        size: n,
        rgba,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{CoverRef, MusicSource};

    fn track(source: MusicSource, playing: bool, since: i64) -> Track {
        Track {
            source,
            title: "Song".into(),
            artist: "Artist".into(),
            device: "Desk".into(),
            duration_secs: 200.0,
            position_secs: 50.0,
            playing,
            since,
            cover: None,
        }
    }

    #[test]
    fn playing_beats_paused_then_later_since_wins() {
        let sp = track(MusicSource::Spotify, true, 100);
        let jf_paused = track(MusicSource::Jellyfin, false, 500);
        assert_eq!(
            pick(Some(&sp), Some(&jf_paused)).unwrap().source,
            MusicSource::Spotify
        );
        assert_eq!(
            pick(Some(&jf_paused), Some(&sp)).unwrap().source,
            MusicSource::Spotify
        );
        let jf_playing = track(MusicSource::Jellyfin, true, 200);
        assert_eq!(
            pick(Some(&sp), Some(&jf_playing)).unwrap().source,
            MusicSource::Jellyfin,
            "both playing: the later start"
        );
        let sp_paused = track(MusicSource::Spotify, false, 600);
        assert_eq!(
            pick(Some(&sp_paused), Some(&jf_paused)).unwrap().source,
            MusicSource::Spotify,
            "both paused: the later activity"
        );
        assert_eq!(pick(None, Some(&jf_paused)).unwrap().since, 500);
        assert!(pick(None, None).is_none());
    }

    #[test]
    fn position_ticks_while_playing_only() {
        let p = track(MusicSource::Spotify, true, 0);
        assert!((position(&p, 10.0, 14.5) - 54.5).abs() < 1e-9);
        assert!((position(&p, 10.0, 5.0) - 50.0).abs() < 1e-9, "never back");
        assert!((position(&p, 0.0, 1000.0) - 200.0).abs() < 1e-9, "clamped");
        let paused = track(MusicSource::Spotify, false, 0);
        assert!((position(&paused, 10.0, 99.0) - 50.0).abs() < 1e-9);
    }

    #[test]
    fn since_carries_over_for_the_same_track_and_state() {
        let prev = track(MusicSource::Spotify, true, 100);
        let mut next = track(MusicSource::Spotify, true, 0);
        stamp_since(Some(&prev), &mut next, 900);
        assert_eq!(next.since, 100, "same track still playing");
        let mut paused = track(MusicSource::Spotify, false, 0);
        stamp_since(Some(&prev), &mut paused, 900);
        assert_eq!(paused.since, 900, "paused now");
        let mut other = track(MusicSource::Spotify, true, 0);
        other.title = "Other".into();
        stamp_since(Some(&prev), &mut other, 950);
        assert_eq!(other.since, 950, "track changed");
        let mut first = track(MusicSource::Spotify, true, 0);
        stamp_since(None, &mut first, 990);
        assert_eq!(first.since, 990);
    }

    #[test]
    fn played_ago_wording() {
        assert_eq!(fmt_played_ago(-5), "just now");
        assert_eq!(fmt_played_ago(59), "just now");
        assert_eq!(fmt_played_ago(12 * 60 + 30), "12 min ago");
        assert_eq!(fmt_played_ago(2 * 3600 + 5), "2 h ago");
        assert_eq!(fmt_played_ago(3 * 86_400), "3 d ago");
    }

    #[test]
    fn demo_cover_is_a_square_gradient_and_refs_compare_by_key() {
        let c = demo_cover("a", Color::hex(0xff0000), Color::hex(0x0000ff));
        assert_eq!(c.size, COVER_PX);
        assert_eq!(c.rgba.len(), (COVER_PX * COVER_PX * 4) as usize);
        assert!(c.rgba.chunks(4).all(|p| p[3] == 255), "opaque");
        assert!(c.rgba[0] > c.rgba[2], "top left leans to a");
        let n = c.rgba.len();
        assert!(c.rgba[n - 2] > c.rgba[n - 4], "bottom right leans to b");
        let same = CoverRef::new(demo_cover("a", Color::hex(0), Color::hex(0)));
        assert_eq!(CoverRef::new(c.clone()), same, "equal by key");
        assert_ne!(
            CoverRef::new(c),
            CoverRef::new(demo_cover("b", Color::hex(0), Color::hex(0)))
        );
    }
}
