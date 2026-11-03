//! Night mode phases: fade the screens to black, fade a dim moon in, hold it,
//! and fade it out again in the morning. Pure, so every transition is tested.

use rackscreen_core::anim::{clamp01, Easing, Secs};

/// Fade to black when the night starts.
pub const NIGHT_FADE_SECS: Secs = 1.0;
/// Fade of the moon icon, in at night and out in the morning.
pub const NIGHT_ICON_FADE_SECS: Secs = 1.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NightPhase {
    /// Normal screens.
    Day,
    /// The screens fade to black.
    FadingOut { since: Secs },
    /// Black; the moon fades in.
    IconIn { since: Secs },
    /// The moon, still.
    Icon,
    /// The moon fades out; then the screens resume.
    IconOut { since: Secs },
}

/// What the screens show this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NightFrame {
    /// The normal scene.
    Day,
    /// The normal scene, darkened: 1.0 is black.
    Fade(f32),
    /// The night scene with the moon at this alpha.
    Icon(f32),
}

/// Progress of a phase that began at `since` and lasts `secs`, 0..=1.
fn progress(since: Secs, now: Secs, secs: Secs) -> f32 {
    clamp01(((now - since) / secs) as f32)
}

/// Quantised like the role crossfade, so equal frames are skipped.
fn q64(v: f32) -> f32 {
    (v * 64.0).round() / 64.0
}

impl NightPhase {
    /// The phase after this tick, given whether it is night now.
    pub fn step(self, night: bool, now: Secs) -> NightPhase {
        use NightPhase::*;
        let s = NIGHT_ICON_FADE_SECS;
        match (self, night) {
            (Day, false) => Day,
            (Day, true) => FadingOut { since: now },
            (FadingOut { .. }, false) => Day,
            (FadingOut { since }, true) => {
                if now - since >= NIGHT_FADE_SECS {
                    IconIn { since: now }
                } else {
                    self
                }
            }
            // The curve is symmetric (ease(1 - t) = 1 - ease(t)), so starting
            // the other fade `1 - p` in keeps the alpha where it is.
            (IconIn { since }, false) => IconOut {
                since: now - (1.0 - progress(since, now, s) as Secs) * s,
            },
            (IconIn { since }, true) => {
                if now - since >= s {
                    Icon
                } else {
                    self
                }
            }
            (Icon, true) => Icon,
            (Icon, false) => IconOut { since: now },
            (IconOut { since }, true) => IconIn {
                since: now - (1.0 - progress(since, now, s) as Secs) * s,
            },
            (IconOut { since }, false) => {
                if now - since >= s {
                    Day
                } else {
                    self
                }
            }
        }
    }

    /// What the screens show in this phase at `now`.
    pub fn frame(self, now: Secs) -> NightFrame {
        use NightPhase::*;
        let ease = |since| Easing::InOutCubic.apply(progress(since, now, NIGHT_ICON_FADE_SECS));
        match self {
            Day => NightFrame::Day,
            FadingOut { since } => NightFrame::Fade(q64(progress(since, now, NIGHT_FADE_SECS))),
            IconIn { since } => NightFrame::Icon(q64(ease(since))),
            Icon => NightFrame::Icon(1.0),
            IconOut { since } => NightFrame::Icon(q64(1.0 - ease(since))),
        }
    }

    /// The screens show the night scene (the moon, at some alpha).
    pub fn shows_icon(self) -> bool {
        matches!(
            self,
            NightPhase::IconIn { .. } | NightPhase::Icon | NightPhase::IconOut { .. }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use NightPhase::*;

    fn q64(v: f32) -> bool {
        (v * 64.0).fract() == 0.0
    }

    #[test]
    fn day_stays_day_and_night_starts_the_fade() {
        assert_eq!(Day.step(false, 5.0), Day);
        assert_eq!(Day.step(true, 5.0), FadingOut { since: 5.0 });
        assert_eq!(Day.frame(5.0), NightFrame::Day);
    }

    #[test]
    fn fade_runs_to_black_then_the_icon_fades_in() {
        let p = FadingOut { since: 10.0 };
        assert_eq!(p.frame(10.0), NightFrame::Fade(0.0));
        assert_eq!(p.frame(10.5), NightFrame::Fade(0.5));
        assert_eq!(p.frame(12.0), NightFrame::Fade(1.0));
        assert_eq!(p.step(true, 10.9), p);
        assert_eq!(p.step(true, 11.0), IconIn { since: 11.0 });
    }

    #[test]
    fn night_ending_mid_fade_goes_straight_back_to_day() {
        assert_eq!(FadingOut { since: 10.0 }.step(false, 10.4), Day);
    }

    #[test]
    fn icon_fades_in_eased_then_holds() {
        let p = IconIn { since: 20.0 };
        assert_eq!(p.frame(20.0), NightFrame::Icon(0.0));
        assert_eq!(p.frame(20.5), NightFrame::Icon(0.5));
        // in-out cubic: slow start
        match p.frame(20.25) {
            NightFrame::Icon(a) => assert_eq!(a, 0.0625),
            f => panic!("{f:?}"),
        }
        assert_eq!(p.step(true, 20.9), p);
        assert_eq!(p.step(true, 21.0), Icon);
        assert_eq!(Icon.frame(99.0), NightFrame::Icon(1.0));
        assert_eq!(Icon.step(true, 99.0), Icon);
    }

    #[test]
    fn morning_during_the_icon_fades_it_out_then_day() {
        let p = Icon.step(false, 30.0);
        assert_eq!(p, IconOut { since: 30.0 });
        assert_eq!(p.frame(30.0), NightFrame::Icon(1.0));
        assert_eq!(p.frame(30.5), NightFrame::Icon(0.5));
        assert_eq!(p.step(false, 30.9), p);
        assert_eq!(p.step(false, 31.0), Day);
    }

    #[test]
    fn morning_mid_icon_in_fades_out_from_where_it_is() {
        // a quarter of the way in: alpha 0.0625; out continues from there
        let p = IconIn { since: 20.0 }.step(false, 20.25);
        assert_eq!(p, IconOut { since: 19.5 });
        assert_eq!(p.frame(20.25), NightFrame::Icon(0.0625));
        assert_eq!(p.step(false, 20.4), p);
        assert_eq!(p.step(false, 20.5), Day);
    }

    #[test]
    fn night_again_mid_icon_out_fades_back_in_from_where_it_is() {
        let p = IconOut { since: 30.0 }.step(true, 30.25);
        assert_eq!(p, IconIn { since: 29.5 });
        assert_eq!(p.frame(30.25), IconOut { since: 30.0 }.frame(30.25));
    }

    #[test]
    fn frames_are_quantised_to_1_64() {
        for i in 0..=100 {
            let t = i as f64 * 0.0137;
            for p in [
                FadingOut { since: 0.0 },
                IconIn { since: 0.0 },
                IconOut { since: 0.0 },
            ] {
                match p.frame(t) {
                    NightFrame::Fade(v) | NightFrame::Icon(v) => {
                        assert!(q64(v), "{p:?} at {t}: {v}");
                        assert!((0.0..=1.0).contains(&v));
                    }
                    NightFrame::Day => panic!(),
                }
            }
        }
    }

    #[test]
    fn only_the_icon_phases_show_the_night_scene() {
        assert!(!Day.shows_icon());
        assert!(!FadingOut { since: 0.0 }.shows_icon());
        assert!(IconIn { since: 0.0 }.shows_icon());
        assert!(Icon.shows_icon());
        assert!(IconOut { since: 0.0 }.shows_icon());
    }

    #[test]
    fn a_whole_night_at_30_hz() {
        let mut p = Day;
        let mut seen = vec![];
        let mut last_alpha: Option<f32> = None;
        for i in 0..300 {
            let now = i as f64 / 30.0;
            p = p.step((1.0..6.0).contains(&now), now);
            let kind = std::mem::discriminant(&p);
            if seen.last() != Some(&kind) {
                seen.push(kind);
            }
            // the moon never jumps more than a few 64ths between frames
            if let NightFrame::Icon(a) = p.frame(now) {
                if let Some(b) = last_alpha {
                    assert!((a - b).abs() <= 6.0 / 64.0, "{b} -> {a} at {now}");
                }
                last_alpha = Some(a);
            }
        }
        let want: Vec<_> = [
            Day,
            FadingOut { since: 0.0 },
            IconIn { since: 0.0 },
            Icon,
            IconOut { since: 0.0 },
            Day,
        ]
        .iter()
        .map(std::mem::discriminant)
        .collect();
        assert_eq!(seen, want);
        assert_eq!(p, Day);
    }
}
