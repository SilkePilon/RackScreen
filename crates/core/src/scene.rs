//! Pure scene description: what to draw on one screen at one instant.

use crate::anim::Secs;
use crate::model::Model;
pub use crate::p2::font::FontId;
use crate::p2::screen::compose;
use crate::p2::ui::ScreenUi;
use crate::theme::{Color, Role, BLACK};

/// An axis-aligned clip in screen pixels for text runs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClipRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Drawable {
    Clear(Color),
    Icon {
        name: &'static str,
        cx: f32,
        cy: f32,
        size: f32,
        color: Color,
        alpha: f32,
        scale: f32,
        dy: f32,
    },
    /// A stroked arc with round caps; angles clockwise from 12 o'clock.
    Arc {
        cx: f32,
        cy: f32,
        r: f32,
        width: f32,
        start_deg: f32,
        sweep_deg: f32,
        color: Color,
        alpha: f32,
    },
    RoundRect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        r: f32,
        color: Color,
        alpha: f32,
    },
    Polygon {
        points: Vec<(f32, f32)>,
        color: Color,
        alpha: f32,
    },
    Ellipse {
        cx: f32,
        cy: f32,
        rx: f32,
        ry: f32,
        color: Color,
        alpha: f32,
    },
    /// Open stroked path with round joins and caps.
    Polyline {
        points: Vec<(f32, f32)>,
        width: f32,
        color: Color,
        alpha: f32,
    },
    /// Text placed by core: left edge `x`, baseline `baseline`.
    Run {
        x: f32,
        baseline: f32,
        text: String,
        font: FontId,
        px: f32,
        letter_spacing: f32,
        color: Color,
        alpha: f32,
        clip: Option<ClipRect>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Scene {
    pub items: Vec<Drawable>,
    /// `items[..base_len]` rarely change (clear, ring track, top icon, card
    /// background); the renderer may reuse a cached raster of them.
    pub base_len: usize,
    /// Brightness of the whole frame, 1.0 = normal; below 1 during a screen
    /// change (crossfade via black).
    pub fade: f32,
}

impl Default for Scene {
    fn default() -> Self {
        Scene::new()
    }
}

impl Scene {
    pub fn new() -> Self {
        Self {
            items: vec![Drawable::Clear(BLACK)],
            base_len: 0,
            fade: 1.0,
        }
    }
    pub fn push(&mut self, d: Drawable) {
        self.items.push(d);
    }
}

pub fn role_scene(model: &Model, role: Role, now: Secs, ui: &mut ScreenUi) -> Scene {
    ui.for_role(role);
    match role {
        Role::PowerMix => crate::p2::roles::power_mix::power_mix_scene(model, now, ui),
        _ => {
            let screen = crate::p2::roles::screen(model, role, now)
                .expect("every other role has a P2 builder");
            compose(&screen, ui, now)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{Event, LinkTarget, Torrent};
    use crate::model::Thresholds;

    fn model_with_cpu(pct: f32) -> Model {
        let mut m = Model::new(Thresholds::default());
        m.apply(
            Event::Metrics {
                cpu_pct: pct,
                mem_pct: 10.0,
                mem_used_gb: 1.0,
                mem_total_gb: 8.0,
                hot_cpu: None,
                hot_mem: None,
            },
            0.0,
        );
        m
    }
    fn icons(s: &Scene) -> Vec<&'static str> {
        s.items
            .iter()
            .filter_map(|d| match d {
                Drawable::Icon { name, .. } => Some(*name),
                _ => None,
            })
            .collect()
    }
    fn has_run(s: &Scene, t: &str) -> bool {
        s.items
            .iter()
            .any(|d| matches!(d, Drawable::Run { text, .. } if text == t))
    }
    #[test]
    fn cpu_scene_is_a_p2_tile() {
        let m = model_with_cpu(42.0);
        let s = role_scene(&m, Role::Cpu, 5.0, &mut ScreenUi::new());
        assert!(matches!(s.items[0], Drawable::Clear(_)));
        assert_eq!(icons(&s), vec!["ms-memory"], "the top icon and no other");
    }
    #[test]
    fn health_scene_switches_to_torrent_mode() {
        let mut m = Model::new(Thresholds::default());
        m.apply(
            Event::NodeSnapshot {
                ready: 2,
                total: 2,
                not_ready: vec![],
            },
            0.0,
        );
        let nodes = role_scene(&m, Role::Health, 0.0, &mut ScreenUi::new());
        assert_eq!(icons(&nodes), vec!["ms-monitor_heart"]);
        assert!(!has_run(&nodes, "dl-a"));
        m.apply(
            Event::Link {
                target: LinkTarget::QBittorrent,
                up: true,
            },
            0.0,
        );
        m.apply(
            Event::Torrents(vec![Torrent {
                name: "dl-a".into(),
                progress: 50.0,
                eta_secs: 60,
                speed_bps: 2_097_152,
            }]),
            0.0,
        );
        let dl = role_scene(&m, Role::Health, 1.0, &mut ScreenUi::new());
        assert_eq!(icons(&dl), vec!["ms-monitor_heart"], "same role, same icon");
        assert!(
            has_run(&dl, "Downloads · dl-a"),
            "Downloads and the torrent's name in the pill"
        );
    }
}
