//! `Model::scene`: which scene a physical screen shows at one instant.

use crate::anim::Secs;
use crate::gif::GifState;
use crate::p2::screen::compose;
use crate::p2::status::{connecting_scene, no_data_screen, NoData};
use crate::p2::ui::ScreenUi;
use crate::scene::{role_scene, Blend, Drawable, Scene, ScreenFrame, WorldRef};
use crate::screens::Half;
use crate::shows::schedule::PanelView;
use crate::theme::Role;
use crate::transition::Kind;

/// The frame at `t` of a transition in `kind` from `from` to `to`: both
/// scenes for a two-frame style, else one of them at the style's brightness.
fn blend(
    kind: Kind,
    t: f32,
    from: impl FnOnce() -> Scene,
    to: impl FnOnce() -> Scene,
) -> ScreenFrame {
    if kind.style.uses_both_frames() {
        return ScreenFrame {
            scene: from(),
            blend: Some(Blend { to: to(), kind, t }),
        };
    }
    let (half, brightness) = kind.single(t);
    let mut s = match half {
        Half::Out => from(),
        Half::In => to(),
    };
    s.fade = brightness;
    s.into()
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
            Role::PowerMix | Role::Renewable => !(link.electricity && self.electricity().have),
            Role::Ups => !(link.prom && self.ups().have),
            Role::Net => !(link.prom && self.net().have),
            Role::Deploys => !(link.argocd && self.apps().have),
            Role::GhActivity => !(link.github && self.github().have),
            Role::Weather | Role::Wind => !(link.weather && self.weather().have),
            Role::Aqi => !(link.weather && self.air().have),
            Role::Rain => !(link.rain && self.rain().have),
            Role::Sun | Role::Moon => !self.sky().have,
            // the globe needs the ephemeris to reach now; the pass alone draws nothing
            Role::Iss => !(self.iss().have && self.iss_position().is_some()),
            Role::Traffic => !(link.prom && self.traffic().have),
            Role::Restarts => !(link.prom && self.restarts().have),
            Role::Uptime => !(link.prom && self.uptime().have),
            Role::Fans => !(link.prom && self.fans().have),
            Role::Mains => !(link.prom && self.mains().have),
            Role::Load => !(link.prom && self.load().have),
            Role::Media => !(link.prom && self.media().have),
            Role::Ci => !(link.github && self.ci().have),
            Role::Jellyfin => !self.jellyfin().have,
            Role::Music => !self.music().have,
            Role::Printer => !self.printer().have,
            Role::Ha(i) => !self.ha(i).is_some_and(|h| h.have),
            Role::Gif(i) => !matches!(self.gif(i), GifState::Ready(_)),
            // without a picture it draws the ISS globe, with that role's states
            Role::Sen => false,
        }
    }

    /// Cluster roles show the connecting scene while the API link is down.
    fn wants_connecting(&self, role: Role) -> bool {
        role.is_cluster() && !self.link().api
    }

    /// Scene for one screen at one instant: the frame's outgoing scene (the
    /// whole frame for a single-scene transition). Tests and goldens.
    pub fn scene(&self, screen: usize, now: Secs, ui: &mut ScreenUi) -> Scene {
        self.frame(screen, now, ui).scene
    }

    /// What `screen` shows at `now`; the only call the render loop makes.
    /// `ui` is that screen's widget state; it resets itself on a role change.
    pub fn frame(&self, screen: usize, now: Secs, ui: &mut ScreenUi) -> ScreenFrame {
        let state = match self.screen(screen) {
            Some(s) => s,
            None => return connecting_scene("ms-sync", ui, now).into(),
        };
        let role = state.current();
        if let Some(run) = self.show_run() {
            if let Some(view) = run.view(now, self.show_position(screen)) {
                let world = || {
                    let mut s = Scene::new();
                    s.push(Drawable::World {
                        frame: WorldRef(self.show_world(run, now)),
                        x0: self.show_x0(screen),
                    });
                    s
                };
                return match view {
                    PanelView::Role => self.scene_for_role(role, now, ui).into(),
                    PanelView::Show => world().into(),
                    PanelView::Mix { t, from_show: true } => blend(run.spec.kind, t, world, || {
                        self.scene_for_role(role, now, ui)
                    }),
                    PanelView::Mix {
                        t,
                        from_show: false,
                    } => blend(
                        run.spec.kind,
                        t,
                        || self.scene_for_role(role, now, ui),
                        world,
                    ),
                };
            }
        }
        if let Some(tr) = state.transition(now) {
            // both halves are live: the old role keeps animating in its own
            // widget state while the new one starts fresh
            let mut out = ui.take_outgoing(tr.from);
            let frame = blend(
                tr.kind,
                tr.t,
                || self.scene_for_role(tr.from, now, &mut out),
                || self.scene_for_role(tr.to, now, ui),
            );
            ui.put_outgoing(out);
            return frame;
        }
        ui.end_transition();
        self.scene_for_role(role, now, ui).into()
    }

    /// A GIF role: the current frame over the whole screen once decoded, the
    /// status screen before that and after a failure. The frame clock starts
    /// when the GIF first shows.
    pub(crate) fn gif_scene(&self, index: u16, now: Secs, ui: &mut ScreenUi) -> Scene {
        let role = Role::Gif(index);
        ui.for_role(role);
        match self.gif(index) {
            GifState::Ready(frames) => {
                let t0 = ui.since(("gif", 0), now);
                let mut s = Scene::new();
                s.push(Drawable::Image {
                    frame: frames.0.frame_at(now - t0) as u16,
                    frames: frames.clone(),
                });
                s
            }
            GifState::Loading => compose(&no_data_screen(role, NoData::Loading), ui, now),
            GifState::Failed(_) => compose(&no_data_screen(role, NoData::GifFailed), ui, now),
        }
    }

    /// The SEN role: the live picture over the whole screen, nothing on top.
    /// Without one (the stream is not live, and no other role could take the
    /// screen) it is the ISS globe screen.
    pub(crate) fn sen_scene(&self, now: Secs, ui: &mut ScreenUi) -> Scene {
        match self.sen_frame() {
            Some(frame) => {
                ui.for_role(Role::Sen);
                // the picture covers the screen: no clear under it
                Scene {
                    items: vec![Drawable::Video(frame.clone())],
                    base_len: 0,
                    fade: 1.0,
                }
            }
            None => self.scene_for_role(Role::Iss, now, ui),
        }
    }

    /// Role scene without a transition (tests, goldens, calibrate).
    pub fn scene_for_role(&self, role: Role, now: Secs, ui: &mut ScreenUi) -> Scene {
        if let Role::Gif(index) = role {
            return self.gif_scene(index, now, ui);
        }
        if role == Role::Sen {
            return self.sen_scene(now, ui);
        }
        if self.wants_connecting(role) {
            return connecting_scene(role.icon(), ui, now);
        }
        if role == Role::Jellyfin && !self.jellyfin_configured() {
            ui.for_role(role);
            return compose(&no_data_screen(role, NoData::Config), ui, now);
        }
        let unconfigured = match role {
            Role::Printer => !self.printer_configured(),
            Role::Ha(i) => !self.ha_configured() || self.ha(i).is_none(),
            _ => false,
        };
        if unconfigured {
            ui.for_role(role);
            return compose(&no_data_screen(role, NoData::Config), ui, now);
        }
        if role == Role::Music && !self.music_configured() {
            ui.for_role(role);
            return compose(&no_data_screen(role, NoData::Config), ui, now);
        }
        if self.link().prom && self.exporter_missing(role) {
            ui.for_role(role);
            return compose(&no_data_screen(role, NoData::Exporter), ui, now);
        }
        if !self.needs_data(role) {
            return role_scene(self, role, now, ui);
        }
        let electricity = matches!(role, Role::PowerMix | Role::Renewable);
        let why = if role.is_sky() && !self.location_present() {
            NoData::Location
        } else if (matches!(role, Role::GhActivity | Role::Ci) && !self.github_token_present())
            || (electricity && !self.token_present())
        {
            NoData::Token
        } else {
            NoData::Waiting
        };
        ui.for_role(role);
        compose(&no_data_screen(role, why), ui, now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{Event, LinkTarget};
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

    fn icon_of(s: &Scene) -> Option<&'static str> {
        s.items.iter().find_map(|d| match d {
            Drawable::Icon { name, .. } if name.starts_with("ms-") => Some(*name),
            _ => None,
        })
    }

    fn pill_of(s: &Scene) -> Option<String> {
        // the pill text is the first run with more than one character after the hero glyphs
        let mut runs = s.items.iter().filter_map(|d| match d {
            Drawable::Run { text, .. } => Some(text.clone()),
            _ => None,
        });
        runs.find(|t| t.chars().count() > 1 && t != "—")
    }

    /// The connecting scene: the runner arc and the "Connecting to Kubernetes API" pill.
    fn connecting(s: &Scene) -> bool {
        s.items.iter().any(
            |d| matches!(d, Drawable::Run { text, .. } if text == "Connecting to Kubernetes API"),
        )
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
            updated_at: "2026-09-07T12:00:00Z".into(),
        }
    }

    fn arcs(s: &Scene) -> usize {
        s.items
            .iter()
            .filter(|d| matches!(d, Drawable::Arc { .. }))
            .count()
    }

    fn for_role(m: &Model, role: Role, now: f64) -> Scene {
        m.scene_for_role(role, now, &mut ScreenUi::new())
    }

    #[test]
    fn connecting_when_api_down() {
        let m = Model::new(Thresholds::default());
        let s = m.scene(Role::Cpu.index(), 0.0, &mut ScreenUi::new());
        assert!(connecting(&s));
        assert_eq!(icon_of(&s), Some("ms-memory"), "the role's icon");
    }

    #[test]
    fn no_data_until_metrics_arrive() {
        let mut m = Model::new(Thresholds::default());
        m.apply(link(LinkTarget::K8sApi, true), 0.0);
        let s = m.scene(Role::Cpu.index(), 0.0, &mut ScreenUi::new());
        assert_eq!(pill_of(&s).as_deref(), Some("Waiting for data"));
        let s = m.scene(Role::Pods.index(), 0.0, &mut ScreenUi::new());
        assert_eq!(pill_of(&s).as_deref(), Some("Waiting for data"));
    }

    /// Tick at 30 Hz until screen 0 starts a transition; returns its start time.
    fn tick_until_transition(m: &mut Model) -> Secs {
        for i in 0..=1000 {
            m.tick(i as f64 / 30.0);
            if let Some(t0) = m.screen(0).unwrap().transition_started() {
                return t0;
            }
        }
        panic!("no transition within 33 s");
    }

    #[test]
    fn cycling_screen_fades_between_roles() {
        let mut m = ready_model();
        m.set_screens(vec![vec![Role::Cpu, Role::Mem]], vec![3.0]);
        let t0 = tick_until_transition(&mut m);
        let mut ui = ScreenUi::new();
        let out = m.scene(0, t0 + 0.05, &mut ui);
        assert_eq!(icon_of(&out), Some("ms-memory"));
        assert!(out.fade > 0.0 && out.fade < 1.0, "outgoing half fading");
        let inc = m.scene(0, t0 + 0.35, &mut ui);
        assert_eq!(icon_of(&inc), Some("ms-memory_alt"));
        assert!(inc.fade > 0.0 && inc.fade < 1.0, "incoming half fading in");
        m.tick(t0 + 0.5);
        assert_eq!(m.current_role(0), Role::Mem);
        let settled = m.scene(0, t0 + 0.6, &mut ui);
        assert_eq!(settled.fade, 1.0, "settled on mem");
        assert_eq!(
            ui.outgoing(Role::Cpu).role(),
            None,
            "the outgoing slot was dropped"
        );
    }

    #[test]
    fn new_roles_show_no_data_and_out_of_range_screen_connects() {
        let mut m = ready_model();
        m.set_screens(vec![vec![Role::Thermal]], vec![15.0]);
        let s = m.scene(0, 1.0, &mut ScreenUi::new());
        assert_eq!(pill_of(&s).as_deref(), Some("Waiting for data"));
        assert_eq!(s.fade, 1.0);
        let s = m.scene(7, 1.0, &mut ScreenUi::new());
        assert!(connecting(&s));
        assert_eq!(icon_of(&s), Some("ms-sync"));
    }

    #[test]
    fn electricity_roles_go_back_to_no_data_when_the_link_drops() {
        let mut m = ready_model();
        m.set_token_present(true);
        m.apply(electricity_event(), 0.0);
        m.apply(link(LinkTarget::Electricity, true), 0.0);
        let s = for_role(&m, Role::PowerMix, 5.0);
        assert!(arcs(&s) >= 20, "power mix ring lit with the link up");
        m.apply(link(LinkTarget::Electricity, false), 6.0);
        for role in [Role::PowerMix, Role::Renewable] {
            assert_eq!(
                pill_of(&for_role(&m, role, 6.0)).as_deref(),
                Some("Waiting for data"),
                "{role:?}: stale data is not shown as live"
            );
        }
        m.set_token_present(false);
        assert_eq!(
            pill_of(&for_role(&m, Role::PowerMix, 6.0)).as_deref(),
            Some("Token needed")
        );
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
        assert_eq!(
            icon_of(&for_role(&m, Role::Thermal, 1.0)),
            Some("ms-thermostat")
        );
        assert_eq!(
            icon_of(&for_role(&m, Role::Storage, 1.0)),
            Some("ms-hard_drive")
        );
        m.apply(link(LinkTarget::Prometheus, false), 2.0);
        for role in [Role::Thermal, Role::Storage] {
            assert_eq!(
                pill_of(&for_role(&m, role, 2.0)).as_deref(),
                Some("Waiting for data")
            );
        }
    }

    #[test]
    fn unconfigured_roles_show_pin_or_key() {
        let m = ready_model();
        assert_eq!(
            pill_of(&for_role(&m, Role::Weather, 1.0)).as_deref(),
            Some("Location needed")
        );
        assert_eq!(
            pill_of(&for_role(&m, Role::GhActivity, 1.0)).as_deref(),
            Some("Token needed")
        );
        assert_eq!(
            pill_of(&for_role(&m, Role::Ups, 1.0)).as_deref(),
            Some("Waiting for data")
        );
        assert_eq!(
            pill_of(&for_role(&m, Role::Ci, 1.0)).as_deref(),
            Some("Token needed")
        );
        let mut m = ready_model();
        m.set_location_present(true);
        m.set_github_token_present(true);
        assert_eq!(
            pill_of(&for_role(&m, Role::Weather, 1.0)).as_deref(),
            Some("Waiting for data")
        );
        assert_eq!(
            pill_of(&for_role(&m, Role::GhActivity, 1.0)).as_deref(),
            Some("Waiting for data")
        );
        assert_eq!(
            pill_of(&for_role(&m, Role::Ci, 1.0)).as_deref(),
            Some("Waiting for data")
        );
    }

    #[test]
    fn electricity_roles_do_not_need_the_api_link() {
        let mut m = Model::new(Thresholds::default());
        m.set_token_present(true);
        m.set_screens(vec![vec![Role::PowerMix], vec![Role::Cpu]], vec![15.0; 2]);
        m.apply(electricity_event(), 0.0);
        m.apply(link(LinkTarget::Electricity, true), 0.0);
        assert!(!m.link().api);
        let s = m.scene(0, 5.0, &mut ScreenUi::new());
        assert!(!connecting(&s), "power mix ignores the API link");
        assert!(arcs(&s) >= 20);
        assert!(
            connecting(&m.scene(1, 5.0, &mut ScreenUi::new())),
            "cpu still connects"
        );
    }

    #[test]
    fn transition_into_a_cluster_role_with_api_down_shows_connecting() {
        let mut m = Model::new(Thresholds::default());
        m.set_token_present(true);
        m.set_screens(vec![vec![Role::PowerMix, Role::Cpu]], vec![3.0]);
        m.apply(electricity_event(), 0.0);
        m.apply(link(LinkTarget::Electricity, true), 0.0);
        let t0 = tick_until_transition(&mut m);
        assert!(
            connecting(&m.scene(0, t0 + 0.3, &mut ScreenUi::new())),
            "incoming cpu half is the connecting scene"
        );
    }

    #[test]
    fn music_checks_config_then_waits_then_draws_itself() {
        let mut m = ready_model();
        assert_eq!(
            pill_of(&for_role(&m, Role::Music, 1.0)).as_deref(),
            Some("Check config")
        );
        m.set_music_configured(true);
        assert_eq!(
            pill_of(&for_role(&m, Role::Music, 1.0)).as_deref(),
            Some("Waiting for data")
        );
        m.apply(
            Event::Music {
                source: crate::event::MusicSource::Spotify,
                track: None,
            },
            1.0,
        );
        let s = for_role(&m, Role::Music, 2.0);
        assert_eq!(pill_of(&s).as_deref(), Some("nothing playing"));
        assert_eq!(icon_of(&s), Some("ms-music_note"));
    }

    #[test]
    fn new_roles_wait_jellyfin_checks_config_and_exporters_can_be_missing() {
        let mut m = ready_model();
        m.set_github_token_present(true);
        for role in [Role::Media, Role::Traffic, Role::Load, Role::Ci] {
            assert_eq!(
                pill_of(&for_role(&m, role, 1.0)).as_deref(),
                Some("Waiting for data"),
                "{role:?}"
            );
        }
        assert_eq!(
            pill_of(&for_role(&m, Role::Jellyfin, 1.0)).as_deref(),
            Some("Check config")
        );
        m.set_jellyfin_configured(true);
        assert_eq!(
            pill_of(&for_role(&m, Role::Jellyfin, 1.0)).as_deref(),
            Some("Waiting for data")
        );
        m.apply(Event::NoExporter { role: Role::Fans }, 1.0);
        assert_eq!(
            pill_of(&for_role(&m, Role::Fans, 1.0)).as_deref(),
            Some("No exporter")
        );
        m.apply(link(LinkTarget::Prometheus, false), 2.0);
        assert_eq!(
            pill_of(&for_role(&m, Role::Fans, 2.0)).as_deref(),
            Some("Waiting for data"),
            "a missing exporter only means something while Prometheus answers"
        );
        assert!(!connecting(&m.scene_for_role(
            Role::Uptime,
            2.0,
            &mut ScreenUi::new()
        )));
        m.apply(link(LinkTarget::K8sApi, false), 3.0);
        assert!(connecting(&for_role(&m, Role::Uptime, 3.0)), "cluster role");
        assert!(
            !connecting(&for_role(&m, Role::Ci, 3.0)),
            "ci is not a cluster role"
        );
    }

    fn show_model(order: Vec<usize>) -> Model {
        use crate::shows::{schedule::ShowScheduler, world::Layout, SHOWS};
        let mut m = ready_model();
        m.set_shows(
            ShowScheduler::new(true, 15, SHOWS.iter().collect()),
            Layout { panels: 4, gap: 46 },
            order,
        );
        m
    }

    fn world_of(s: &Scene) -> Option<(std::sync::Arc<crate::shows::world::WorldFrame>, u32)> {
        match s.items.as_slice() {
            [Drawable::Clear(c), Drawable::World { frame, x0 }] if *c == crate::theme::BLACK => {
                Some((frame.0.clone(), *x0))
            }
            _ => None,
        }
    }

    #[test]
    fn a_show_fades_roles_out_plays_the_world_and_fades_roles_back() {
        use crate::shows::schedule::{Request, SHOW_FADE_SECS};
        let order = vec![2, 0, 1, 3];
        let mut m = show_model(order.clone());
        let mut ui: Vec<ScreenUi> = (0..4).map(|_| ScreenUi::new()).collect();
        assert!(m.request_show(Request::Id(crate::shows::by_id("freight").unwrap())));
        m.tick(10.0);
        let secs = crate::shows::by_id("freight").unwrap().secs;
        // roles out: the role scene, dimmed like the crossfade's out half
        m.tick(10.1);
        let s = m.scene(0, 10.1, &mut ui[0]);
        assert!(world_of(&s).is_none());
        assert_eq!(s.fade, crate::screens::fade(0.25).1);
        assert_eq!(
            icon_of(&s),
            icon_of(&m.scene_for_role(Role::Cpu, 10.1, &mut ScreenUi::new()))
        );
        // playing: one world, each screen at its position's slice
        let t = 10.0 + SHOW_FADE_SECS + 1.04; // mid-frame
        m.tick(t);
        let layout = crate::shows::world::Layout { panels: 4, gap: 46 };
        let mut frames = Vec::new();
        for (screen, u) in ui.iter_mut().enumerate() {
            let s = m.scene(screen, t, u);
            assert_eq!(s.fade, 1.0);
            let (frame, x0) = world_of(&s).expect("world scene");
            let pos = order.iter().position(|&c| c == screen).unwrap() as u32;
            assert_eq!(x0, layout.x0(pos), "screen {screen}");
            assert_eq!(frame.width, layout.width());
            frames.push(frame);
        }
        assert!(
            frames
                .windows(2)
                .all(|w| std::sync::Arc::ptr_eq(&w[0], &w[1])),
            "one Arc for all"
        );
        // same 12 fps frame on the next tick: the same Arc, so workers skip it
        m.tick(t + 0.01);
        let again = world_of(&m.scene(1, t + 0.01, &mut ui[1])).unwrap().0;
        assert!(std::sync::Arc::ptr_eq(&again, &frames[0]));
        m.tick(t + 0.1);
        let next = world_of(&m.scene(1, t + 0.1, &mut ui[1])).unwrap().0;
        assert!(
            !std::sync::Arc::ptr_eq(&next, &frames[0]),
            "a new frame, a new Arc"
        );
        // show out: the world fades
        let t = 10.0 + SHOW_FADE_SECS + secs + 0.1;
        m.tick(t);
        let s = m.scene(3, t, &mut ui[3]);
        assert!(world_of(&s).is_some());
        assert_eq!(s.fade, crate::screens::fade(0.25).1);
        // roles in
        let t = 10.0 + 2.0 * SHOW_FADE_SECS + secs + 0.1;
        m.tick(t);
        let s = m.scene(3, t, &mut ui[3]);
        assert!(world_of(&s).is_none());
        assert_eq!(s.fade, crate::screens::fade(0.75).1);
        // over: plain roles again
        let t = 10.0 + 3.0 * SHOW_FADE_SECS + secs + 0.1;
        m.tick(t);
        let s = m.scene(3, t, &mut ui[3]);
        assert!(world_of(&s).is_none());
        assert_eq!(s.fade, 1.0);
    }

    #[test]
    fn default_order_is_config_order() {
        use crate::shows::schedule::Request;
        let mut m = show_model(vec![]);
        assert!(m.request_show(Request::Random));
        m.tick(0.0);
        m.tick(1.0);
        let layout = crate::shows::world::Layout { panels: 4, gap: 46 };
        for screen in 0..4 {
            let (_, x0) = world_of(&m.scene(screen, 1.0, &mut ScreenUi::new())).unwrap();
            assert_eq!(x0, layout.x0(screen as u32));
        }
    }

    fn gif(frames: usize) -> crate::gif::GifRef {
        use crate::gif::{GifFrames, GifRef, FRAME_BYTES};
        GifRef(std::sync::Arc::new(
            GifFrames::new(
                (0..frames)
                    .map(|i| (vec![i as u8; FRAME_BYTES], 100))
                    .collect(),
            )
            .unwrap(),
        ))
    }

    fn image_of(s: &Scene) -> Option<(crate::gif::GifRef, u16)> {
        s.items.iter().find_map(|d| match d {
            Drawable::Image { frames, frame } => Some((frames.clone(), *frame)),
            _ => None,
        })
    }

    #[test]
    fn gif_loading_and_failed_use_the_status_screen() {
        let mut m = Model::new(Thresholds::default());
        let s = for_role(&m, Role::Gif(1), 0.0);
        assert_eq!(icon_of(&s), Some("ms-gif_box"));
        assert_eq!(pill_of(&s).as_deref(), Some("Loading"));
        assert!(image_of(&s).is_none());
        m.apply(
            Event::GifFailed {
                index: 1,
                reason: "404".into(),
            },
            0.0,
        );
        let s = for_role(&m, Role::Gif(1), 0.0);
        assert_eq!(icon_of(&s), Some("ms-gif_box"));
        assert_eq!(pill_of(&s).as_deref(), Some("GIF failed"));
        assert_eq!(
            pill_of(&for_role(&m, Role::Gif(0), 0.0)).as_deref(),
            Some("Loading"),
            "other GIFs are untouched"
        );
    }

    #[test]
    fn a_ready_gif_fills_the_screen_without_ring_or_icon() {
        let mut m = Model::new(Thresholds::default());
        let g = gif(3);
        m.apply(
            Event::Gif {
                index: 0,
                frames: g.clone(),
            },
            0.0,
        );
        let s = for_role(&m, Role::Gif(0), 5.0);
        assert_eq!(s.items.len(), 2, "{:?}", s.items);
        assert!(matches!(s.items[0], Drawable::Clear(_)));
        assert_eq!(image_of(&s), Some((g, 0)));
        assert_eq!(icon_of(&s), None);
        assert_eq!(arcs(&s), 0);
    }

    #[test]
    fn gif_frames_follow_the_time_on_screen_and_loop() {
        let mut m = Model::new(Thresholds::default());
        m.apply(
            Event::Gif {
                index: 0,
                frames: gif(3),
            },
            0.0,
        );
        let mut ui = ScreenUi::new();
        let at = |m: &Model, ui: &mut ScreenUi, t: f64| {
            image_of(&m.scene_for_role(Role::Gif(0), t, ui)).unwrap().1
        };
        assert_eq!(at(&m, &mut ui, 50.0), 0, "starts at the first frame");
        assert_eq!(at(&m, &mut ui, 50.15), 1);
        assert_eq!(at(&m, &mut ui, 50.25), 2);
        assert_eq!(at(&m, &mut ui, 50.35), 0, "loops");
        assert_eq!(
            m.scene_for_role(Role::Gif(0), 50.36, &mut ui),
            m.scene_for_role(Role::Gif(0), 50.37, &mut ui),
            "same frame, equal scenes: the render worker skips it"
        );
        // another role and back: starts over
        m.scene_for_role(Role::Cpu, 51.0, &mut ui);
        assert_eq!(at(&m, &mut ui, 52.15), 0);
    }

    #[test]
    fn a_gif_screen_crossfades_like_any_role() {
        let mut m = Model::new(Thresholds::default());
        m.apply(
            Event::Gif {
                index: 0,
                frames: gif(2),
            },
            0.0,
        );
        m.set_screens(vec![vec![Role::Gif(0), Role::Cpu]], vec![5.0]);
        let mut ui = ScreenUi::new();
        let s = m.scene(0, 1.0, &mut ui);
        assert!(image_of(&s).is_some());
        assert_eq!(s.fade, 1.0);
        m.tick(5.1);
        let s = m.scene(0, 5.15, &mut ui);
        assert!(image_of(&s).is_some(), "fading out");
        assert!(s.fade < 1.0);
    }

    #[test]
    fn the_iss_globe_waits_for_a_pass_answer_and_an_ephemeris_that_reaches_now() {
        use crate::event::IssTrack;
        let globe = |s: &Scene| s.items.iter().any(|d| matches!(d, Drawable::Globe { .. }));
        let scene = |m: &Model| m.scene_for_role(Role::Iss, 0.0, &mut ScreenUi::new());
        let mut m = Model::new(Thresholds::default());
        m.set_location_present(true);
        m.set_unix_now(10_000);
        let track = IssTrack {
            t0: 10_000 - 3600,
            step_secs: 20,
            points: (0..481).map(|i| (i as f32 * 0.1, i as f32 * 0.2)).collect(),
            home: (52.0, 5.0),
        };
        m.apply(Event::IssTrack(track), 0.0);
        assert!(!globe(&scene(&m)), "no pass answer yet: still waiting");
        m.apply(Event::IssPass(None), 0.0);
        let s = scene(&m);
        assert!(globe(&s), "no pass in 24 h still shows the globe");
        assert!(s
            .items
            .iter()
            .any(|d| matches!(d, Drawable::Run { text, .. } if text == "No pass in 24 h")));
        // the clock ran past the ephemeris (the source stopped): waiting again
        m.set_unix_now(10_000 + 6001);
        assert!(!globe(&scene(&m)));
        // a pass alone, with no ephemeris, is not enough either
        let mut bare = Model::new(Thresholds::default());
        bare.apply(Event::IssPass(None), 0.0);
        assert!(!globe(&scene(&bare)));
    }
    mod sen {
        use super::*;
        use crate::event::{IssTrack, SenEvent};
        use crate::model::{SEN_HOLD_SECS, SEN_WARMUP_SECS};
        use crate::screens::Entry;
        use crate::video::{VideoFrame, VideoRef};
        use std::sync::Arc;

        const UNIX: i64 = 1_790_000_000;

        fn frame() -> VideoRef {
            VideoRef(Arc::new(VideoFrame::new()))
        }

        /// A station that hangs over one spot: under the sun, or opposite it.
        fn track(day: bool) -> IssTrack {
            let (lat, lon) = crate::globe::subsolar(UNIX);
            let point = if day {
                (lat as f32, lon as f32)
            } else {
                (-lat as f32, (lon + 180.0) as f32)
            };
            IssTrack {
                t0: UNIX - 3600,
                step_secs: 20,
                points: vec![point; 481],
                home: (52.0, 5.0),
            }
        }

        fn model(roles: &[(Role, Secs)]) -> Model {
            let mut m = Model::new(Thresholds::default());
            m.set_unix_now(UNIX);
            m.apply(Event::IssTrack(track(true)), 0.0);
            m.apply(Event::IssPass(None), 0.0);
            m.set_screen_entries(vec![roles
                .iter()
                .map(|(r, s)| Entry::new(*r, *s))
                .collect()]);
            m
        }

        /// Frames at 15 fps from `from` to `to`, ticking the model at 30 Hz.
        fn stream(m: &mut Model, from: Secs, to: Secs, frames: bool) {
            let mut t = from;
            let mut n = 0;
            while t < to {
                if frames && n % 2 == 0 {
                    m.apply(Event::Sen(SenEvent::Frame(frame())), t);
                }
                m.tick(t);
                n += 1;
                t = from + n as f64 / 30.0;
            }
        }

        fn video_of(s: &Scene) -> Option<VideoRef> {
            match s.items.as_slice() {
                [Drawable::Video(f)] => Some(f.clone()),
                _ => None,
            }
        }

        fn has_globe(s: &Scene) -> bool {
            s.items.iter().any(|d| matches!(d, Drawable::Globe { .. }))
        }

        #[test]
        fn sen_is_available_only_while_live_with_fresh_frames_by_day() {
            let mut m = model(&[(Role::Cpu, 10.0), (Role::Sen, 10.0)]);
            assert!(!m.role_available(Role::Sen), "nothing came yet");
            assert!(Role::ALL
                .iter()
                .all(|r| *r == Role::Sen || m.role_available(*r)));
            let f = frame();
            m.apply(Event::Sen(SenEvent::Frame(f.clone())), 1.0);
            m.tick(1.0);
            assert!(!m.role_available(Role::Sen), "a frame, but not live");
            m.apply(Event::Sen(SenEvent::Live(true)), 1.0);
            assert!(m.role_available(Role::Sen));
            assert_eq!(m.sen_frame(), Some(&f));
            // the frames stop coming: not available once the last is stale
            m.tick(1.0 + SEN_HOLD_SECS + 0.1);
            assert!(!m.role_available(Role::Sen));
            assert_eq!(m.sen_frame(), None);
            m.apply(Event::Sen(SenEvent::Frame(frame())), 3.0);
            m.tick(3.0);
            assert!(m.role_available(Role::Sen), "they came back");
            // the stream leaves live footage
            m.apply(Event::Sen(SenEvent::Live(false)), 3.1);
            assert!(!m.role_available(Role::Sen));
            assert!(m.sen_frame().is_some(), "the last picture is still there");
            // the station goes into the Earth's shadow
            m.apply(Event::Sen(SenEvent::Live(true)), 3.2);
            assert!(m.role_available(Role::Sen));
            m.apply(Event::IssTrack(track(false)), 3.2);
            assert!(!m.role_available(Role::Sen));
            // stopped: nothing is held
            m.apply(Event::Sen(SenEvent::Stopped), 3.3);
            assert_eq!(m.sen(), &crate::model::SenState::default());
        }

        #[test]
        fn the_stream_is_wanted_shortly_before_the_slot_and_while_shown() {
            let mut m = model(&[(Role::Cpu, 20.0), (Role::Sen, 20.0)]);
            m.tick(0.0);
            assert!(!m.sen_wanted(0.0));
            assert!(!m.sen_wanted(20.0 - SEN_WARMUP_SECS - 0.5));
            assert!(m.sen_wanted(20.0 - SEN_WARMUP_SECS + 0.5), "warming up");
            // the stream goes live in time: the screen changes to it
            stream(&mut m, 12.5, 14.0, true);
            m.apply(Event::Sen(SenEvent::Live(true)), 14.0);
            stream(&mut m, 14.0, 21.0, true);
            assert_eq!(m.current_role(0), Role::Sen);
            assert!(m.sen_wanted(21.0), "on screen");
            // and back to cpu after its dwell; then it is not wanted for a while
            stream(&mut m, 21.0, 42.0, true);
            assert_eq!(m.current_role(0), Role::Cpu);
            assert!(!m.sen_wanted(42.0));
        }

        #[test]
        fn a_stream_that_is_not_live_at_its_slot_is_skipped() {
            let mut m = model(&[(Role::Cpu, 10.0), (Role::Sen, 10.0), (Role::Mem, 10.0)]);
            assert!(m.sen_wanted(5.0), "it would be next");
            // it runs but shows the slate: no frames, not live
            stream(&mut m, 0.0, 11.0, false);
            assert_eq!(m.current_role(0), Role::Mem, "sen was skipped");
            assert!(!m.sen_wanted(11.0), "mem's dwell has just begun");
        }

        #[test]
        fn sen_leaves_the_screen_as_soon_as_it_stops_being_live() {
            let mut m = model(&[(Role::Sen, 60.0), (Role::Cpu, 10.0)]);
            m.apply(Event::Sen(SenEvent::Live(true)), 0.0);
            stream(&mut m, 0.0, 5.0, true);
            assert_eq!(m.current_role(0), Role::Sen);
            let mut ui = ScreenUi::new();
            assert!(video_of(&m.scene(0, 5.0, &mut ui)).is_some());
            assert!(m.screen(0).unwrap().transition(5.0).is_none());
            // the slate comes up 5 s into a 60 s dwell
            m.apply(Event::Sen(SenEvent::Live(false)), 5.0);
            m.tick(5.0);
            assert!(
                m.screen(0).unwrap().transition(5.0).is_some(),
                "leaving now"
            );
            // it fades out from the last picture, not from the globe
            let s = m.scene(0, 5.1, &mut ui);
            assert!(video_of(&s).is_some());
            assert!(s.fade < 1.0);
            stream(&mut m, 5.0, 6.0, false);
            assert_eq!(m.current_role(0), Role::Cpu);
            // while cpu shows and sen is not live, cpu stays past its dwell
            stream(&mut m, 6.0, 30.0, false);
            assert_eq!(m.current_role(0), Role::Cpu);
        }

        #[test]
        fn a_screen_with_only_sen_shows_the_globe_until_the_stream_is_live() {
            let mut m = model(&[(Role::Sen, 30.0)]);
            let mut ui = ScreenUi::new();
            m.tick(0.0);
            assert!(m.sen_wanted(0.0), "it keeps looking for live footage");
            let s = m.scene(0, 0.0, &mut ui);
            assert!(has_globe(&s) && video_of(&s).is_none());
            m.apply(Event::Sen(SenEvent::Live(true)), 1.0);
            let f = frame();
            m.apply(Event::Sen(SenEvent::Frame(f.clone())), 1.0);
            m.tick(1.0);
            let s = m.scene(0, 1.0, &mut ui);
            assert_eq!(video_of(&s), Some(f), "the picture and nothing else");
            assert_eq!(s.base_len, 0);
            // the same frame again is the same scene: the workers skip it
            assert_eq!(m.scene(0, 1.03, &mut ui), s);
            // not live any more: the globe is back once the picture is stale
            m.apply(Event::Sen(SenEvent::Live(false)), 2.0);
            m.tick(1.0 + SEN_HOLD_SECS + 0.1);
            assert!(has_globe(&m.scene(0, 3.0, &mut ui)));
            // on the night side the stream is not wanted at all
            m.apply(Event::IssTrack(track(false)), 3.0);
            assert!(!m.sen_wanted(3.0));
            assert!(has_globe(&m.scene(0, 3.0, &mut ui)));
        }

        #[test]
        fn the_night_ends_a_stream_delay_after_the_station_leaves_the_shadow() {
            use crate::model::SEN_STREAM_DELAY_SECS;
            // in shadow until UNIX, sunlit after
            let (lat, lon) = crate::globe::subsolar(UNIX);
            let mut tr = track(true);
            for (i, p) in tr.points.iter_mut().enumerate() {
                if tr.t0 + i as i64 * 20 < UNIX {
                    *p = (-lat as f32, (lon + 180.0) as f32);
                }
            }
            let mut m = model(&[(Role::Sen, 30.0)]);
            m.apply(Event::IssTrack(tr), 0.0);
            m.set_unix_now(UNIX - 100);
            assert!(!m.sen_daylight());
            m.set_unix_now(UNIX + SEN_STREAM_DELAY_SECS - 25);
            assert!(!m.sen_daylight(), "the stream still shows the night");
            m.set_unix_now(UNIX + SEN_STREAM_DELAY_SECS + 25);
            assert!(m.sen_daylight());
            // without an ephemeris the picture decides
            let bare = Model::new(Thresholds::default());
            assert!(bare.sen_daylight());
        }

        #[test]
        fn without_iss_data_the_fallback_is_the_iss_no_data_screen() {
            let mut m = Model::new(Thresholds::default());
            m.set_screen_entries(vec![vec![Entry::new(Role::Sen, 30.0)]]);
            let s = for_role(&m, Role::Sen, 0.0);
            assert_eq!(icon_of(&s), Some("ms-satellite_alt"));
            assert!(!has_globe(&s));
        }
    }
}
