//! Pure scene description: what to draw on one screen at one instant.

use crate::anim::{breathe, pulse, Secs};
use crate::event::Torrent;
use crate::format::{fmt_eta, fmt_speed};
use crate::model::Model;
use crate::theme::layout::*;
use crate::theme::{
    Color, Role, BADGE_FILL, BLACK, BLUE, DIM_GREY, GREEN, GREY, RED, VIOLET, WHITE,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SegState {
    Off,
    On(Color, f32),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Drawable {
    Clear(Color),
    Ring {
        cx: f32,
        cy: f32,
        radius: f32,
        n: usize,
        states: Vec<SegState>,
    },
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
    Badge {
        cx: f32,
        cy: f32,
        w: f32,
        h: f32,
        radius: f32,
        stroke: Color,
        fill: Color,
        text: String,
        text_px: f32,
        text_color: Color,
        alpha: f32,
    },
    Ripple {
        cx: f32,
        cy: f32,
        r: f32,
        thickness: f32,
        color: Color,
        alpha: f32,
    },
    Dots {
        cx: f32,
        cy: f32,
        spacing: f32,
        r: f32,
        colors: Vec<Color>,
    },
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scene {
    pub items: Vec<Drawable>,
}

impl Scene {
    pub fn new() -> Self {
        Self {
            items: vec![Drawable::Clear(BLACK)],
        }
    }
    pub fn push(&mut self, d: Drawable) {
        self.items.push(d);
    }
    /// The outermost ring (first Ring pushed).
    pub fn ring_mut(&mut self) -> Option<&mut Drawable> {
        self.items
            .iter_mut()
            .find(|d| matches!(d, Drawable::Ring { .. }))
    }
    /// The role icon (first Icon pushed).
    pub fn main_icon_mut(&mut self) -> Option<&mut Drawable> {
        self.items
            .iter_mut()
            .find(|d| matches!(d, Drawable::Icon { .. }))
    }
    pub fn lit_count(&self) -> usize {
        match self
            .items
            .iter()
            .find(|d| matches!(d, Drawable::Ring { .. }))
        {
            Some(Drawable::Ring { states, .. }) => states
                .iter()
                .filter(|s| matches!(s, SegState::On(..)))
                .count(),
            _ => 0,
        }
    }
}

fn icon(name: &'static str, cy: f32, size: f32, color: Color, alpha: f32) -> Drawable {
    Drawable::Icon {
        name,
        cx: CX,
        cy,
        size,
        color,
        alpha,
        scale: 1.0,
        dy: 0.0,
    }
}

fn badge(cy: f32, stroke: Color, text: String) -> Drawable {
    Drawable::Badge {
        cx: CX,
        cy,
        w: BADGE_W,
        h: BADGE_H,
        radius: BADGE_RADIUS,
        stroke,
        fill: BADGE_FILL,
        text,
        text_px: BADGE_TEXT_PX,
        text_color: WHITE,
        alpha: 1.0,
    }
}

fn ring(radius: f32, states: Vec<SegState>) -> Drawable {
    let n = states.len();
    Drawable::Ring {
        cx: CX,
        cy: CY,
        radius,
        n,
        states,
    }
}

/// Segment count for a ring of the given radius (60 at r=102, fewer inside).
pub fn seg_count(radius: f32) -> usize {
    ((SEG_N as f32) * radius / RING_R).round() as usize
}

/// Lit segments for a percentage; the last lit one breathes.
pub fn ring_states(pct: f32, accent: Color, n: usize, now: Secs) -> Vec<SegState> {
    let lit = ((pct.clamp(0.0, 100.0) / 100.0) * n as f32).round() as usize;
    (0..n)
        .map(|i| {
            if i + 1 < lit {
                SegState::On(accent, 1.0)
            } else if i + 1 == lit {
                SegState::On(accent, breathe(now, 2.4))
            } else {
                SegState::Off
            }
        })
        .collect()
}

/// Segments for the PODS ring: running (blue), pending (pulsing dim blue), failed (red).
pub fn pod_segments(
    running: f32,
    pending: u32,
    failed: u32,
    total: u32,
    now: Secs,
) -> Vec<SegState> {
    let n = SEG_N;
    if total == 0 {
        return vec![SegState::Off; n];
    }
    let scale = if total as usize <= n {
        1.0
    } else {
        n as f32 / total as f32
    };
    let run = (running.max(0.0) * scale).round() as usize;
    let pend = ((pending as f32) * scale).round() as usize;
    let mut fail = ((failed as f32) * scale).round() as usize;
    if failed > 0 {
        fail = fail.max(1);
    }
    let pend_alpha = 0.3 + 0.5 * pulse(now, 1.2);
    let mut out = vec![SegState::Off; n];
    let mut i = 0;
    for _ in 0..run.min(n) {
        out[i] = SegState::On(BLUE, 1.0);
        i += 1;
    }
    if run > 0 && i > 0 {
        out[i - 1] = SegState::On(BLUE, breathe(now, 2.4));
    }
    for _ in 0..pend.min(n - i) {
        out[i] = SegState::On(BLUE, pend_alpha);
        i += 1;
    }
    for _ in 0..fail.min(n - i) {
        out[i] = SegState::On(RED, 1.0);
        i += 1;
    }
    out
}

fn heartbeat(now: Secs) -> f32 {
    let phase = now % 4.0;
    if phase < 0.3 {
        ((phase / 0.3) * std::f64::consts::PI).sin() as f32
    } else {
        0.0
    }
}

pub fn role_scene(model: &Model, role: Role, now: Secs) -> Scene {
    match role {
        Role::Cpu => {
            let pct = model.smooth_cpu(now);
            let mut s = Scene::new();
            s.push(ring(RING_R, ring_states(pct, role.accent(), SEG_N, now)));
            s.push(icon(
                role.icon(),
                ICON_CY,
                ICON_SIZE,
                WHITE,
                0.6 + 0.4 * pulse(now, 3.5),
            ));
            s.push(badge(BADGE_CY, role.accent(), format!("{:.0}%", pct)));
            s
        }
        Role::Mem => {
            let pct = model.smooth_mem(now);
            let mut s = Scene::new();
            s.push(ring(RING_R, ring_states(pct, role.accent(), SEG_N, now)));
            s.push(icon(role.icon(), ICON_CY, ICON_SIZE, WHITE, 1.0));
            s.push(badge(BADGE_CY, role.accent(), format!("{:.0}%", pct)));
            s
        }
        Role::Pods => {
            let st = model.state();
            let running = model.smooth_pods(now);
            let mut s = Scene::new();
            s.push(ring(
                RING_R,
                pod_segments(running, st.pods_pending, st.pods_failed, st.pods_total, now),
            ));
            let mut ic = icon(role.icon(), ICON_CY, ICON_SIZE, WHITE, 1.0);
            if let Drawable::Icon { dy, .. } = &mut ic {
                *dy = -3.0 * pulse(now, 2.6);
            }
            s.push(ic);
            s.push(badge(
                BADGE_CY,
                role.accent(),
                format!("{}", running.round() as u32),
            ));
            if st.pods_failed > 0 {
                s.push(Drawable::Dots {
                    cx: CX,
                    cy: MARKER_CY,
                    spacing: 0.0,
                    r: 3.0,
                    colors: vec![RED.with_alpha(breathe(now, 2.4))],
                });
            }
            s
        }
        Role::Health => {
            if model.torrent_mode() {
                return torrent_scene(&model.state().torrents, now);
            }
            let st = model.state();
            let mut s = Scene::new();
            let states = if st.nodes_total == 0 {
                vec![SegState::Off; SEG_N]
            } else {
                let ready_pct = st.nodes_ready as f32 / st.nodes_total as f32 * 100.0;
                let mut v = ring_states(ready_pct, GREEN, SEG_N, now);
                let lit = v.iter().filter(|x| matches!(x, SegState::On(..))).count();
                for x in v.iter_mut().skip(lit) {
                    *x = SegState::On(RED, 0.9);
                }
                v
            };
            s.push(ring(RING_R, states));
            let alert = !st.alerts.is_empty();
            let mut heart = icon(
                role.icon(),
                ICON_CY,
                ICON_SIZE,
                if alert { RED } else { WHITE },
                1.0,
            );
            if let Drawable::Icon { scale, .. } = &mut heart {
                *scale = 1.0 + 0.12 * heartbeat(now);
            }
            s.push(heart);
            s.push(badge(
                BADGE_CY,
                if alert { RED } else { GREEN },
                String::new(),
            ));
            let total = st.nodes_total as usize;
            if total > 0 {
                let ready = st.nodes_ready as usize;
                let colors = (0..total)
                    .map(|i| if i < ready { GREEN } else { RED })
                    .collect();
                s.push(Drawable::Dots {
                    cx: CX,
                    cy: BADGE_CY,
                    spacing: DOT_SPACING,
                    r: DOT_R,
                    colors,
                });
            }
            if alert {
                s.push(Drawable::Dots {
                    cx: CX,
                    cy: MARKER_CY,
                    spacing: 0.0,
                    r: 3.0,
                    colors: vec![RED.with_alpha(breathe(now, 2.4))],
                });
            }
            s
        }
    }
}

const TORRENT_ACCENTS: [Color; 3] = [GREEN, BLUE, VIOLET];

pub fn torrent_scene(torrents: &[Torrent], now: Secs) -> Scene {
    let mut sorted: Vec<&Torrent> = torrents.iter().collect();
    sorted.sort_by(|a, b| {
        b.progress
            .partial_cmp(&a.progress)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut s = Scene::new();
    for (i, t) in sorted.iter().take(3).enumerate() {
        let r = TORRENT_RADII[i];
        s.push(ring(
            r,
            ring_states(t.progress, TORRENT_ACCENTS[i], seg_count(r), now),
        ));
    }
    let mut ic = icon("download", TORRENT_ICON_CY, TORRENT_ICON_SIZE, WHITE, 1.0);
    if let Drawable::Icon { dy, .. } = &mut ic {
        *dy = -2.0 + 4.0 * pulse(now, 1.6);
    }
    s.push(ic);
    let window = (now / 5.0).floor();
    let frac = now - window * 5.0;
    let text = if (window as i64) % 2 == 0 {
        fmt_speed(sorted.iter().map(|t| t.speed_bps).sum())
    } else {
        let eta = sorted
            .iter()
            .map(|t| t.eta_secs)
            .filter(|e| (0..864_000).contains(e))
            .min();
        fmt_eta(eta.unwrap_or(-1))
    };
    let mut b = badge(TORRENT_BADGE_CY, GREEN, text);
    if let Drawable::Badge { alpha, .. } = &mut b {
        *alpha = ((frac / 0.25) as f32).min(1.0);
    }
    s.push(b);
    s
}

pub fn connecting_scene(now: Secs) -> Scene {
    let n = SEG_N;
    let head = ((now / 2.4) * n as f64).floor() as usize % n;
    let tail = [1.0, 0.7, 0.45, 0.25];
    let mut states = vec![SegState::Off; n];
    for (k, a) in tail.iter().enumerate() {
        let idx = (head + n - k) % n;
        states[idx] = SegState::On(GREY, *a);
    }
    let mut s = Scene::new();
    s.push(ring(RING_R, states));
    s.push(icon("plug-zap", CY, BIG_ICON_SIZE, GREY, breathe(now, 2.4)));
    s
}

pub fn no_data_scene(now: Secs) -> Scene {
    let n = SEG_N;
    let mut states = vec![SegState::On(DIM_GREY, 1.0); n];
    let phase = now % 4.0;
    if phase < 1.0 {
        let idx = ((phase * n as f64).floor() as usize).min(n - 1);
        states[idx] = SegState::On(GREY, 1.0);
    }
    let mut s = Scene::new();
    s.push(ring(RING_R, states));
    s.push(icon(
        "cloud-off",
        CY,
        BIG_ICON_SIZE,
        Color::hex(0x666666),
        1.0,
    ));
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{Event, LinkTarget};
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

    #[test]
    fn ring_states_lights_rounded_share() {
        let v = ring_states(42.0, BLUE, 60, 0.0);
        assert_eq!(
            v.iter().filter(|s| matches!(s, SegState::On(..))).count(),
            25
        );
        assert!(
            matches!(v[24], SegState::On(_, a) if a < 1.0),
            "last lit breathes"
        );
        assert!(matches!(v[0], SegState::On(_, a) if a == 1.0));
        assert_eq!(
            ring_states(0.0, BLUE, 60, 0.0)
                .iter()
                .filter(|s| matches!(s, SegState::On(..)))
                .count(),
            0
        );
        assert_eq!(ring_states(100.0, BLUE, 60, 0.0).len(), 60);
    }

    #[test]
    fn pod_segments_order_and_colours() {
        let v = pod_segments(50.0, 2, 1, 53, 0.0);
        assert!(matches!(v[0], SegState::On(c, _) if c == BLUE));
        assert!(
            matches!(v[50], SegState::On(c, a) if c == BLUE && a < 1.0),
            "pending dim"
        );
        assert!(matches!(v[52], SegState::On(c, _) if c == RED));
        assert_eq!(v[53], SegState::Off);
    }

    #[test]
    fn pod_segments_scale_when_more_than_sixty() {
        let v = pod_segments(180.0, 0, 1, 200, 0.0);
        let lit = v.iter().filter(|s| matches!(s, SegState::On(..))).count();
        assert_eq!(lit, 55, "54 running + at least 1 failed");
        assert!(matches!(v[54], SegState::On(c, _) if c == RED));
    }

    #[test]
    fn cpu_scene_after_settling_shows_value() {
        let m = model_with_cpu(42.0);
        let s = role_scene(&m, Role::Cpu, 5.0);
        assert_eq!(s.lit_count(), 25);
        let badge_text = s.items.iter().find_map(|d| match d {
            Drawable::Badge { text, .. } => Some(text.clone()),
            _ => None,
        });
        assert_eq!(badge_text.as_deref(), Some("42%"));
        assert!(matches!(s.items[0], Drawable::Clear(_)));
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
        let s = role_scene(&m, Role::Health, 0.0);
        assert_eq!(s.lit_count(), 60);
        let dots = s
            .items
            .iter()
            .filter(|d| matches!(d, Drawable::Dots { .. }))
            .count();
        assert_eq!(dots, 1);
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
                    name: "a".into(),
                    progress: 50.0,
                    eta_secs: 60,
                    speed_bps: 2_097_152,
                },
                Torrent {
                    name: "b".into(),
                    progress: 90.0,
                    eta_secs: 30,
                    speed_bps: 1_048_576,
                },
            ]),
            0.0,
        );
        let s = role_scene(&m, Role::Health, 0.0);
        let rings: Vec<_> = s
            .items
            .iter()
            .filter(|d| matches!(d, Drawable::Ring { .. }))
            .collect();
        assert_eq!(rings.len(), 2);
        if let Drawable::Ring { radius, .. } = rings[0] {
            assert_eq!(*radius, 102.0);
        }
        assert_eq!(s.lit_count(), 54, "outer ring is the 90% torrent");
        let text = s.items.iter().find_map(|d| match d {
            Drawable::Badge { text, .. } => Some(text.clone()),
            _ => None,
        });
        assert_eq!(text.as_deref(), Some("3.0M"));
        let s2 = role_scene(&m, Role::Health, 5.5);
        let text2 = s2.items.iter().find_map(|d| match d {
            Drawable::Badge { text, .. } => Some(text.clone()),
            _ => None,
        });
        assert_eq!(text2.as_deref(), Some("30s"));
    }

    #[test]
    fn health_not_ready_shows_red_tail() {
        let mut m = Model::new(Thresholds::default());
        m.apply(
            Event::NodeSnapshot {
                ready: 3,
                total: 4,
                not_ready: vec!["n4".into()],
            },
            0.0,
        );
        let s = role_scene(&m, Role::Health, 0.0);
        assert_eq!(s.lit_count(), 60);
        if let Some(Drawable::Ring { states, .. }) =
            s.items.iter().find(|d| matches!(d, Drawable::Ring { .. }))
        {
            assert!(matches!(states[59], SegState::On(c, _) if c == RED));
            assert!(matches!(states[0], SegState::On(c, _) if c == GREEN));
        }
    }

    #[test]
    fn connecting_has_comet_and_big_icon() {
        let s = connecting_scene(0.0);
        assert_eq!(s.lit_count(), 4);
        assert!(
            matches!(s.items[2], Drawable::Icon { name: "plug-zap", size, .. } if size == BIG_ICON_SIZE)
        );
        let s2 = connecting_scene(0.6);
        assert_eq!(s2.lit_count(), 4);
        assert_ne!(s, s2);
    }

    #[test]
    fn no_data_is_dim_full_ring() {
        let s = no_data_scene(2.0);
        assert_eq!(s.lit_count(), 60);
        assert!(matches!(
            s.items[2],
            Drawable::Icon {
                name: "cloud-off",
                ..
            }
        ));
    }
}
