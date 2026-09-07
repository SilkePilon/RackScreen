//! Splash overlays and sweep scenes layered on top of role scenes.

use crate::anim::{pulse, Easing, Secs};
use crate::fx::{Splash, Sweep, SweepPhase, SPLASH_SECS};
use crate::scene::{
    connecting_scene, no_data_scene, no_data_scene_with, role_scene, Drawable, Scene, SegState,
};
use crate::theme::layout::*;
use crate::theme::{Role, WHITE};

const RIPPLE_SECS: f32 = 0.3;
const ICON_SWAP_START: f32 = 0.1;
const ICON_SWAP_SECS: f32 = 0.4;
const FADE_BACK_START: f32 = 2.0;
const FLASH_STAGGER: f32 = 0.01;
const FLASH_SECS: f32 = 0.3;

fn unit(x: f32) -> f32 {
    x.clamp(0.0, 1.0)
}

pub fn splash_overlay(mut base: Scene, splash: &Splash, now: Secs) -> Scene {
    let e = splash.elapsed(now);
    if e >= SPLASH_SECS as f32 {
        return base;
    }
    let color = splash.kind.color();

    // 1. ring flash wave
    if let Some(Drawable::Ring { states, .. }) = base.ring_mut() {
        for (i, st) in states.iter_mut().enumerate() {
            let t0 = i as f32 * FLASH_STAGGER;
            let f = 1.0 - unit((e - t0) / FLASH_SECS);
            if e >= t0 && f > 0.0 {
                let (c, a) = match *st {
                    SegState::On(c, a) => (c, a),
                    SegState::Off => (crate::theme::OFF, 1.0),
                };
                *st = SegState::On(c.mix(color, f), a.max(f));
            }
        }
    }

    // 2. role icon fades out then back in
    let swap = unit((e - ICON_SWAP_START) / ICON_SWAP_SECS);
    let back = unit((e - FADE_BACK_START) / (SPLASH_SECS as f32 - FADE_BACK_START));
    let role_alpha = (1.0 - swap).max(back);
    let (icx, icy, isize) = match base.main_icon_mut() {
        Some(Drawable::Icon {
            alpha,
            cx,
            cy,
            size,
            ..
        }) => {
            *alpha *= role_alpha;
            (*cx, *cy, *size)
        }
        _ => (CX, ICON_CY, ICON_SIZE),
    };

    // 3. event icon pops in with spring, fades out at the end
    if swap > 0.0 {
        let scale = Easing::Spring.apply(swap);
        let alpha = swap.min(1.0 - back);
        base.push(Drawable::Icon {
            name: splash.kind.icon(),
            cx: icx,
            cy: icy,
            size: isize,
            color,
            alpha,
            scale,
            dy: 0.0,
        });
    }

    // 4. ripple
    if e < RIPPLE_SECS {
        let t = e / RIPPLE_SECS;
        base.push(Drawable::Ripple {
            cx: CX,
            cy: CY,
            r: 110.0 * Easing::OutCubic.apply(t),
            thickness: 3.0,
            color,
            alpha: 0.8 * (1.0 - t),
        });
    }

    // 5. collapsed counter in the badge (only badges that carry text; the
    //    thermal dots badge is a frame with no text and stays as it is)
    if splash.count > 1 {
        for d in base.items.iter_mut() {
            if let Drawable::Badge { text, .. } = d {
                if !text.is_empty() {
                    *text = format!("+{}", splash.count);
                }
            }
        }
    }
    base
}

pub fn sweep_scene(sweep: &Sweep, role: Role, phase: SweepPhase, now: Secs) -> Scene {
    let color = sweep.kind.color(role);
    let n = SEG_N;
    let (lit, ring_alpha, icon_alpha) = match phase {
        SweepPhase::Idle => (0, 1.0, 0.0),
        SweepPhase::WipeIn(p) => {
            let p = Easing::OutCubic.apply(p);
            ((p * n as f32).round() as usize, 1.0, p)
        }
        SweepPhase::Hold(_) => (n, 0.6 + 0.4 * pulse(now, 1.2), 1.0),
        SweepPhase::WipeOut(p) => (((1.0 - p) * n as f32).round() as usize, 1.0, 1.0 - p),
    };
    let states = (0..n)
        .map(|i| {
            if i < lit {
                SegState::On(color, ring_alpha)
            } else {
                SegState::Off
            }
        })
        .collect();
    let mut s = Scene::new();
    s.push(Drawable::Ring {
        cx: CX,
        cy: CY,
        radius: RING_R,
        n,
        states,
        pitch_deg: 6.0,
        start_deg: 0.0,
    });
    s.push(Drawable::Icon {
        name: sweep.kind.icon(role),
        cx: CX,
        cy: CY,
        size: BIG_ICON_SIZE,
        color: if matches!(sweep.kind, crate::fx::SweepKind::Boot) {
            WHITE
        } else {
            color
        },
        alpha: icon_alpha,
        scale: 1.0,
        dy: 0.0,
    });
    s
}

impl crate::model::Model {
    /// A role has nothing live to show: its data never arrived, or the link that
    /// feeds it is down (stale data is not shown as live).
    fn needs_data(&self, role: Role) -> bool {
        let st = self.state();
        let link = self.link();
        match role {
            Role::Cpu | Role::Mem => !(link.prom && st.have_metrics),
            Role::Pods => !st.have_pods,
            Role::Health => !st.have_nodes,
            Role::Thermal => !(link.prom && st.have_temps),
            Role::Storage => !(link.prom && st.have_storage),
            Role::PowerMix | Role::Carbon | Role::Renewable => {
                !(link.electricity && self.electricity().have)
            }
            Role::Price => !(link.prices && self.prices().have),
            Role::Ups => !(link.prom && self.ups().have),
            Role::Net => !(link.prom && self.net().have),
            Role::GhActivity
            | Role::Weather
            | Role::Wind
            | Role::Aqi
            | Role::Rain
            | Role::Sun
            | Role::Moon
            | Role::Iss
            | Role::Deploys => true,
        }
    }

    /// Cluster roles show the connecting scene while the API link is down; the
    /// electricity roles do not depend on the cluster at all.
    fn wants_connecting(&self, role: Role) -> bool {
        role.is_cluster() && !self.link().api
    }

    /// Scene for one screen at one instant; the only call the render loop makes.
    pub fn scene(&self, screen: usize, now: Secs) -> Scene {
        let screens = self.screen_count();
        let state = match self.screen(screen) {
            Some(s) => s,
            None => return connecting_scene(now),
        };
        let role = state.current();
        if self.wants_connecting(role) {
            return connecting_scene(now);
        }
        if let Some(sw) = self.fx().sweeps.active() {
            match sw.phase(screen, screens, now) {
                SweepPhase::Idle => {}
                ph => return sweep_scene(sw, role, ph, now),
            }
        }
        if let Some(tr) = state.transition(now) {
            let (zoom, reveal) = crate::screens::transition_transform(tr.t);
            let shown = if tr.t < 0.5 { tr.from } else { tr.to };
            let mut s = if self.wants_connecting(shown) {
                connecting_scene(now)
            } else {
                self.scene_for_role(shown, now)
            };
            s.zoom = zoom;
            s.ring_reveal = reveal;
            return s;
        }
        let base = self.scene_for_role(role, now);
        if self.fx().sweeps.active().is_some() {
            return base;
        }
        match self.fx().splashes[role.index()].active() {
            Some(sp) => splash_overlay(base, sp, now),
            None => base,
        }
    }

    /// Role scene without transition or splash (tests, goldens, calibrate).
    pub fn scene_for_role(&self, role: Role, now: Secs) -> Scene {
        if !self.needs_data(role) {
            return role_scene(self, role, now);
        }
        let electricity = matches!(
            role,
            Role::PowerMix | Role::Price | Role::Carbon | Role::Renewable
        );
        if electricity && !self.token_present() {
            // never configured, rather than a source that went quiet
            no_data_scene_with(now, "key-round")
        } else {
            no_data_scene(now)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{Event, LinkTarget};
    use crate::fx::{SplashKind, SweepKind};
    use crate::model::{Model, Thresholds};

    fn ready_model() -> Model {
        let mut m = Model::new(Thresholds::default());
        m.apply(
            Event::Link {
                target: LinkTarget::K8sApi,
                up: true,
            },
            0.0,
        );
        m.apply(
            Event::Link {
                target: LinkTarget::Prometheus,
                up: true,
            },
            0.0,
        );
        m.apply(
            Event::Metrics {
                cpu_pct: 42.0,
                mem_pct: 60.0,
                mem_used_gb: 1.0,
                mem_total_gb: 8.0,
                hot_cpu: None,
                hot_mem: None,
            },
            0.0,
        );
        m.apply(
            Event::PodSnapshot {
                running: 10,
                pending: 0,
                failed: 0,
                total: 10,
            },
            0.0,
        );
        m.apply(
            Event::NodeSnapshot {
                ready: 2,
                total: 2,
                not_ready: vec![],
            },
            0.0,
        );
        m.set_screens(
            vec![
                vec![Role::Cpu],
                vec![Role::Mem],
                vec![Role::Pods],
                vec![Role::Health],
            ],
            vec![15.0; 4],
        );
        m
    }

    fn icons(s: &Scene) -> Vec<(&'static str, f32, f32)> {
        s.items
            .iter()
            .filter_map(|d| match d {
                Drawable::Icon {
                    name, alpha, scale, ..
                } => Some((*name, *alpha, *scale)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn connecting_when_api_down() {
        let m = Model::new(Thresholds::default());
        let s = m.scene(Role::Cpu.index(), 0.0);
        assert!(icons(&s).iter().any(|(n, ..)| *n == "plug-zap"));
    }

    #[test]
    fn no_data_until_metrics_arrive() {
        let mut m = Model::new(Thresholds::default());
        m.apply(
            Event::Link {
                target: LinkTarget::K8sApi,
                up: true,
            },
            0.0,
        );
        let s = m.scene(Role::Cpu.index(), 0.0);
        assert!(icons(&s).iter().any(|(n, ..)| *n == "cloud-off"));
        let s = m.scene(Role::Pods.index(), 0.0);
        assert!(icons(&s).iter().any(|(n, ..)| *n == "cloud-off"));
    }

    #[test]
    fn splash_swaps_icon_with_overshoot_and_ripple() {
        let mut m = ready_model();
        m.apply(
            Event::PodCrashed {
                ns: "a".into(),
                name: "b".into(),
            },
            1.0,
        );
        m.tick(1.0);
        let s = m.scene(Role::Pods.index(), 1.15);
        assert!(s.items.iter().any(|d| matches!(d, Drawable::Ripple { .. })));
        let s = m.scene(Role::Pods.index(), 1.35);
        let ic = icons(&s);
        let (_, role_alpha, _) = ic.iter().find(|(n, ..)| *n == "box").unwrap();
        let (_, ev_alpha, ev_scale) = ic.iter().find(|(n, ..)| *n == "package-x").unwrap();
        assert!(*role_alpha < 0.6);
        assert!(*ev_alpha > 0.5);
        assert!(*ev_scale > 1.0, "spring overshoot around 60% of swap");
        let s = m.scene(Role::Pods.index(), 3.6);
        assert!(icons(&s).iter().all(|(n, ..)| *n != "package-x"));
    }

    #[test]
    fn splash_counter_in_badge() {
        let mut m = ready_model();
        for _ in 0..3 {
            m.apply(
                Event::PodStarted {
                    ns: "a".into(),
                    name: "b".into(),
                },
                1.0,
            );
        }
        m.tick(1.0);
        let s = m.scene(Role::Pods.index(), 1.5);
        let text = s.items.iter().find_map(|d| match d {
            Drawable::Badge { text, .. } => Some(text.clone()),
            _ => None,
        });
        assert_eq!(text.as_deref(), Some("+3"));
    }

    #[test]
    fn sweep_takes_over_all_screens_and_suppresses_splash() {
        let mut m = ready_model();
        m.apply(
            Event::PodStarted {
                ns: "a".into(),
                name: "b".into(),
            },
            1.0,
        );
        m.apply(
            Event::NodeReady {
                name: "n".into(),
                ready: false,
            },
            1.0,
        );
        m.tick(1.0);
        let s = m.scene(Role::Health.index(), 1.1);
        assert!(icons(&s).iter().any(|(n, ..)| *n == "server-off"));
        assert!(s.lit_count() > 0 && s.lit_count() < 60);
        let s = m.scene(Role::Cpu.index(), 1.1);
        assert!(
            icons(&s).iter().all(|(n, ..)| *n == "cpu"),
            "cpu not yet reached, shows role, no splash"
        );
        let s = m.scene(Role::Cpu.index(), 2.0);
        assert_eq!(s.lit_count(), 60);
        assert!(icons(&s).iter().any(|(n, ..)| *n == "server-off"));
        let s = m.scene(Role::Pods.index(), 2.0);
        assert!(icons(&s).iter().all(|(n, ..)| *n != "package-plus"));
        // tick at ~30 Hz like the real loop so the frozen splash is shifted correctly
        for i in 33..=200 {
            m.tick(i as f64 / 33.0);
        }
        let s = m.scene(Role::Pods.index(), 200.0 / 33.0);
        assert!(
            icons(&s).iter().any(|(n, ..)| *n == "package-plus"),
            "splash resumes after sweep"
        );
    }

    #[test]
    fn cycling_screen_transitions_with_iris() {
        let mut m = ready_model();
        m.set_screens(vec![vec![Role::Cpu, Role::Mem]], vec![3.0]);
        for i in 0..=100 {
            m.tick(i as f64 * 0.033);
        }
        // at 3.3 s: transition started at ~3.0, first half shows cpu shrinking
        let s = m.scene(0, 3.15);
        assert!(s.zoom < 1.0 && s.zoom > 0.0);
        assert!(icons(&s).iter().any(|(n, ..)| *n == "cpu"));
        let s = m.scene(0, 3.4);
        assert!(
            icons(&s).iter().any(|(n, ..)| *n == "memory-stick"),
            "second half shows the incoming role"
        );
        // transition ends at ~3.53 s, then mem dwells; settled by 5.94 s
        for i in 101..=180 {
            m.tick(i as f64 * 0.033);
        }
        assert_eq!(m.current_role(0), Role::Mem);
        assert!(m.screen(0).unwrap().transition(5.94).is_none());
        assert_eq!(m.scene(0, 5.94).zoom, 1.0);
        // the next cycle (3 s after ~3.53 s) wraps back to cpu
        for i in 181..=200 {
            m.tick(i as f64 * 0.033);
        }
        let tr = m
            .screen(0)
            .unwrap()
            .transition(6.6)
            .expect("second cycle started");
        assert_eq!((tr.from, tr.to), (Role::Mem, Role::Cpu));
        assert!(m.scene(0, 6.6).zoom < 1.0);
    }

    #[test]
    fn new_roles_show_no_data_and_out_of_range_screen_connects() {
        let mut m = ready_model();
        m.set_screens(vec![vec![Role::Thermal]], vec![15.0]);
        let s = m.scene(0, 1.0);
        assert!(icons(&s).iter().any(|(n, ..)| *n == "cloud-off"));
        assert_eq!(s.zoom, 1.0);
        let s = m.scene(7, 1.0);
        assert!(icons(&s).iter().any(|(n, ..)| *n == "plug-zap"));
    }

    fn link(target: LinkTarget, up: bool) -> Event {
        Event::Link { target, up }
    }

    fn electricity_event() -> Event {
        Event::Electricity {
            zone: "NL".into(),
            mix_mw: vec![
                (crate::electricity::Source::Solar, 60.0),
                (crate::electricity::Source::Wind, 40.0),
            ],
            renewable_pct: 61.0,
            fossil_free_pct: 73.0,
            carbon_gco2: 214.0,
            updated_at: "2026-09-07T12:00:00Z".into(),
        }
    }

    fn has_icon(s: &Scene, name: &str) -> bool {
        icons(s).iter().any(|(n, ..)| *n == name)
    }

    #[test]
    fn electricity_roles_go_back_to_no_data_when_the_link_drops() {
        let mut m = ready_model();
        m.set_token_present(true);
        m.apply(electricity_event(), 0.0);
        m.apply(link(LinkTarget::Electricity, true), 0.0);
        let s = m.scene_for_role(Role::PowerMix, 5.0);
        assert!(s.lit_count() > 0, "power mix ring lit with the link up");
        assert!(!has_icon(&s, "cloud-off"));
        m.apply(link(LinkTarget::Electricity, false), 6.0);
        let s = m.scene_for_role(Role::PowerMix, 6.0);
        assert!(has_icon(&s, "cloud-off"), "stale mix is not shown as live");
        assert!(has_icon(&m.scene_for_role(Role::Carbon, 6.0), "cloud-off"));
        assert!(has_icon(
            &m.scene_for_role(Role::Renewable, 6.0),
            "cloud-off"
        ));
        // without a token the same outage shows the key instead
        m.set_token_present(false);
        assert!(has_icon(
            &m.scene_for_role(Role::PowerMix, 6.0),
            "key-round"
        ));
    }

    #[test]
    fn price_role_gates_on_the_prices_link() {
        let mut m = ready_model();
        m.set_token_present(true);
        m.apply(
            Event::Prices {
                date: "2026-09-07".into(),
                ct_per_kwh: vec![10.0; 24],
                currency: "EUR".into(),
            },
            0.0,
        );
        m.apply(link(LinkTarget::Prices, true), 0.0);
        assert!(has_icon(&m.scene_for_role(Role::Price, 1.0), "euro"));
        m.apply(link(LinkTarget::Prices, false), 2.0);
        assert!(has_icon(&m.scene_for_role(Role::Price, 2.0), "cloud-off"));
    }

    #[test]
    fn thermal_and_storage_gate_on_the_prometheus_link() {
        let mut m = ready_model();
        m.apply(
            Event::NodeTemps(vec![("n1".into(), 44.0), ("n2".into(), 51.0)]),
            0.0,
        );
        m.apply(
            Event::Storage {
                volumes: vec![],
                used_bytes: 1,
                capacity_bytes: 2,
            },
            0.0,
        );
        assert!(has_icon(
            &m.scene_for_role(Role::Thermal, 1.0),
            "thermometer"
        ));
        assert!(has_icon(&m.scene_for_role(Role::Storage, 1.0), "database"));
        m.apply(link(LinkTarget::Prometheus, false), 2.0);
        assert!(has_icon(&m.scene_for_role(Role::Thermal, 2.0), "cloud-off"));
        assert!(has_icon(&m.scene_for_role(Role::Storage, 2.0), "cloud-off"));
    }

    #[test]
    fn electricity_roles_do_not_need_the_api_link() {
        let mut m = Model::new(Thresholds::default());
        m.set_token_present(true);
        m.set_screens(vec![vec![Role::PowerMix], vec![Role::Cpu]], vec![15.0; 2]);
        m.apply(electricity_event(), 0.0);
        m.apply(link(LinkTarget::Electricity, true), 0.0);
        assert!(!m.link().api);
        let s = m.scene(0, 5.0);
        assert!(!has_icon(&s, "plug-zap"), "power mix ignores the API link");
        assert!(s.lit_count() > 0);
        assert!(has_icon(&m.scene(1, 5.0), "plug-zap"), "cpu still connects");
    }

    #[test]
    fn transition_into_a_cluster_role_with_api_down_shows_connecting() {
        let mut m = Model::new(Thresholds::default());
        m.set_token_present(true);
        m.set_screens(vec![vec![Role::PowerMix, Role::Cpu]], vec![3.0]);
        m.apply(electricity_event(), 0.0);
        m.apply(link(LinkTarget::Electricity, true), 0.0);
        for i in 0..=100 {
            m.tick(i as f64 * 0.033);
        }
        let s = m.scene(0, 3.4);
        assert!(s.zoom < 1.0, "mid transition");
        assert!(
            has_icon(&s, "plug-zap"),
            "incoming cpu half is the connecting scene"
        );
    }

    #[test]
    fn two_screens_with_the_same_role_both_get_the_splash() {
        let mut m = ready_model();
        m.set_screens(vec![vec![Role::Pods], vec![Role::Pods]], vec![15.0; 2]);
        m.apply(
            Event::PodStarted {
                ns: "a".into(),
                name: "b".into(),
            },
            1.0,
        );
        m.tick(1.0);
        assert!(has_icon(&m.scene(0, 1.4), "package-plus"));
        assert!(has_icon(&m.scene(1, 1.4), "package-plus"));
    }

    #[test]
    fn splash_counter_leaves_textless_badges_alone() {
        let mut m = ready_model();
        m.apply(
            Event::NodeTemps(vec![("n1".into(), 44.0), ("n2".into(), 51.0)]),
            0.0,
        );
        m.set_screens(vec![vec![Role::Thermal]], vec![15.0]);
        for _ in 0..3 {
            m.apply(
                Event::HotTemp {
                    node: "n2".into(),
                    celsius: 80.0,
                },
                1.0,
            );
        }
        m.tick(1.0);
        let s = m.scene(0, 1.5);
        let texts: Vec<String> = s
            .items
            .iter()
            .filter_map(|d| match d {
                Drawable::Badge { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect();
        assert!(!texts.is_empty());
        assert!(
            texts.iter().all(|t| t.is_empty() || t == "+3"),
            "the dots badge keeps its empty text, got {texts:?}"
        );
        assert!(
            texts.iter().any(|t| t.is_empty()),
            "thermal dots badge is textless"
        );
    }

    #[test]
    fn boot_uses_role_colours() {
        let sw = Sweep {
            kind: SweepKind::Boot,
            started: 0.0,
        };
        let s = sweep_scene(&sw, Role::Mem, SweepPhase::Hold(0.5), 0.0);
        if let Some(Drawable::Ring { states, .. }) =
            s.items.iter().find(|d| matches!(d, Drawable::Ring { .. }))
        {
            assert!(matches!(states[0], SegState::On(c, _) if c == Role::Mem.accent()));
        }
        assert!(icons(&s).iter().any(|(n, ..)| *n == "memory-stick"));
    }

    #[test]
    fn splash_kinds_have_icons() {
        for k in [
            SplashKind::PodStarted,
            SplashKind::PodCrashed,
            SplashKind::PodGone,
            SplashKind::HotNode,
            SplashKind::HotTemp,
            SplashKind::VolumeDegraded,
            SplashKind::VolumeHealthy,
            SplashKind::TorrentAdded,
        ] {
            assert!(!k.icon().is_empty());
        }
    }
}
