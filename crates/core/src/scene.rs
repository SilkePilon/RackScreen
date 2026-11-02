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
    /// Text set along a circle around the screen centre, upright at the top,
    /// centred on `centre_deg` (clockwise from 12 o'clock).
    CurvedText {
        text: String,
        radius: f32,
        centre_deg: f32,
        px: f32,
        font: FontId,
        letter_spacing: f32,
        color: Color,
        alpha: f32,
    },
}

/// Where one half of a slide sits: vertical offset in px, scale about the
/// centre, opacity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub dy: f32,
    pub scale: f32,
    pub alpha: f32,
}

impl Placement {
    pub const STILL: Placement = Placement {
        dy: 0.0,
        scale: 1.0,
        alpha: 1.0,
    };
}

/// A screen change in progress: both role scenes and where each sits.
#[derive(Clone, Debug, PartialEq)]
pub struct Slide {
    pub out: Scene,
    pub out_at: Placement,
    pub inc: Scene,
    pub inc_at: Placement,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Scene {
    pub items: Vec<Drawable>,
    /// During a screen change: the two halves to draw instead of `items`.
    pub slide: Option<Box<Slide>>,
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
            slide: None,
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
        Role::Face => crate::scene_face::face_scene(model, now),
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
    fn title_of(s: &Scene) -> Option<String> {
        s.items.iter().find_map(|d| match d {
            Drawable::CurvedText { text, .. } => Some(text.clone()),
            _ => None,
        })
    }
    #[test]
    fn cpu_scene_is_a_p2_tile() {
        let m = model_with_cpu(42.0);
        let s = role_scene(&m, Role::Cpu, 5.0, &mut ScreenUi::new());
        assert_eq!(title_of(&s).as_deref(), Some("CPU"));
        assert!(matches!(s.items[0], Drawable::Clear(_)));
        assert!(!s.items.iter().any(|d| matches!(d, Drawable::Icon { .. })));
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
        assert_eq!(
            title_of(&role_scene(&m, Role::Health, 0.0, &mut ScreenUi::new())).as_deref(),
            Some("CLUSTER")
        );
        m.apply(
            Event::Link {
                target: LinkTarget::QBittorrent,
                up: true,
            },
            0.0,
        );
        m.apply(
            Event::Torrents(vec![Torrent {
                name: "a".into(),
                progress: 50.0,
                eta_secs: 60,
                speed_bps: 2_097_152,
            }]),
            0.0,
        );
        assert_eq!(
            title_of(&role_scene(&m, Role::Health, 1.0, &mut ScreenUi::new())).as_deref(),
            Some("DOWNLOADS")
        );
    }
}
