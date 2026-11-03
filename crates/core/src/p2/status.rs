//! What a screen shows without live data: waiting, missing token or
//! location, and the connecting scene while the cluster link is down.

use crate::anim::Secs;
use crate::p2::font::FontId;
use crate::p2::layout::*;
use crate::p2::ring::RingSpec;
use crate::p2::screen::P2Screen;
use crate::p2::text::{middle_baseline, run_centred, top_icon};
use crate::scene::{Drawable, Scene};
use crate::theme::layout::CX;
use crate::theme::p2::{TEXT2, TEXT3, TRACK, WARN};
use crate::theme::Role;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoData {
    /// The source is configured but has not delivered yet, or went quiet.
    Waiting,
    /// An API token is missing from the config.
    Token,
    /// `location.lat` / `lon` are missing from the config.
    Location,
    /// Prometheus answers but has no series for this role.
    Exporter,
    /// A URL or similar the role cannot work without is empty in the config.
    Config,
}

pub fn no_data_screen(role: Role, why: NoData) -> P2Screen {
    let mut s = P2Screen::new(role.title(), "—");
    s.icon = role.icon();
    s.hero_color = TEXT3;
    s.ring = RingSpec::empty();
    s.pill = Some(match why {
        NoData::Waiting => ("Waiting for data".into(), TEXT2),
        NoData::Token => ("Token needed".into(), WARN),
        NoData::Location => ("Location needed".into(), WARN),
        NoData::Exporter => ("No exporter".into(), WARN),
        NoData::Config => ("Check config".into(), WARN),
    });
    s
}

/// One lap of the runner around the track.
pub const CONNECT_LAP_SECS: Secs = 1.6;
pub const CONNECT_RUNNER_FRAC: f32 = 0.12;

/// The cluster link is down: a `TRACK` ring with a short `TEXT2` arc running
/// round it, `icon` in the ring gap and the link's name in the pill.
pub fn connecting_scene(icon: &'static str, now: Secs) -> Scene {
    let mut s = Scene::new();
    let spec = RingSpec::empty();
    s.items.extend(spec.drawables());
    let phase = (now / CONNECT_LAP_SECS).rem_euclid(1.0) as f32;
    let f1 = phase + CONNECT_RUNNER_FRAC;
    s.items.extend(spec.arc(phase, f1.min(1.0), TEXT2, 1.0));
    if f1 > 1.0 {
        s.items.extend(spec.arc(0.0, f1 - 1.0, TEXT2, 1.0));
    }
    s.push(top_icon(icon));
    let text = "Kubernetes API";
    let w = crate::p2::font::measure(FontId::SemiBold, PILL_PX, text, 0.0) + PILL_PAD;
    s.push(Drawable::RoundRect {
        x: CX - w / 2.0,
        y: PILL_CY - PILL_H / 2.0,
        w,
        h: PILL_H,
        r: PILL_R,
        color: TRACK,
        alpha: 1.0,
    });
    s.push(run_centred(
        CX,
        middle_baseline(FontId::SemiBold, PILL_PX, PILL_CY + 0.5),
        text,
        FontId::SemiBold,
        PILL_PX,
        0.0,
        TEXT2,
        1.0,
    ));
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::p2::ring::RingKind;
    use crate::p2::ui::ScreenUi;
    use crate::scene::Drawable;
    use crate::theme::p2::{TEXT2, TEXT3, WARN};
    use crate::theme::Role;

    #[test]
    fn no_data_screens_show_a_dash_and_a_pill() {
        let s = no_data_screen(Role::Cpu, NoData::Waiting);
        assert_eq!(
            (s.icon, s.hero.as_str(), s.hero_color),
            ("ms-memory", "—", TEXT3)
        );
        assert!(matches!(s.ring.kind, RingKind::Empty));
        assert_eq!(s.pill, Some(("Waiting for data".into(), TEXT2)));
        assert!(s.card.is_none() && s.unit.is_none());
        assert_eq!(
            no_data_screen(Role::Price, NoData::Token).pill,
            Some(("Token needed".into(), WARN))
        );
        assert_eq!(
            no_data_screen(Role::Sun, NoData::Location).pill,
            Some(("Location needed".into(), WARN))
        );
        assert_eq!(
            no_data_screen(Role::Weather, NoData::Waiting).icon,
            "ms-partly_cloudy_day"
        );
        assert_eq!(
            no_data_screen(Role::Fans, NoData::Exporter).pill,
            Some(("No exporter".into(), WARN))
        );
        assert_eq!(
            no_data_screen(Role::Jellyfin, NoData::Config),
            P2Screen {
                pill: Some(("Check config".into(), WARN)),
                ..no_data_screen(Role::Jellyfin, NoData::Waiting)
            }
        );
    }

    #[test]
    fn connecting_runs_an_arc_round_the_track() {
        let a = connecting_scene("ms-memory", 0.0);
        let arcs: Vec<(f32, f32)> = a
            .items
            .iter()
            .filter_map(|d| match d {
                Drawable::Arc {
                    start_deg,
                    sweep_deg,
                    ..
                } => Some((*start_deg, *sweep_deg)),
                _ => None,
            })
            .collect();
        assert_eq!(arcs.len(), 2, "track and the runner");
        assert!(
            (arcs[1].1 - 0.12 * RING_SWEEP_DEG).abs() < 1e-3,
            "12 % of the sweep"
        );
        assert!(a
            .items
            .iter()
            .any(|d| matches!(d, Drawable::Icon { name, .. } if *name == "ms-memory")));
        assert!(a
            .items
            .iter()
            .any(|d| matches!(d, Drawable::Run { text, .. } if text == "Kubernetes API")));
        let b = connecting_scene("ms-memory", 0.8);
        let half = match &b.items[2] {
            Drawable::Arc { start_deg, .. } => *start_deg,
            _ => panic!("arc"),
        };
        assert!(
            (half - (arcs[1].0 + RING_SWEEP_DEG / 2.0)).abs() < 1e-3,
            "half a lap in 0.8 s"
        );
        assert_eq!(
            connecting_scene("ms-memory", 1.6),
            connecting_scene("ms-memory", 0.0),
            "one lap every 1.6 s"
        );
        let mut ui = ScreenUi::new();
        let composed =
            crate::p2::screen::compose(&no_data_screen(Role::Mem, NoData::Waiting), &mut ui, 0.0);
        assert!(composed
            .items
            .iter()
            .any(|d| matches!(d, Drawable::Run { text, .. } if text == "—")));
    }
}
