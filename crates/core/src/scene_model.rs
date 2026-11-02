//! `Model::scene`: which scene a physical screen shows at one instant.

use crate::anim::Secs;
use crate::p2::screen::compose;
use crate::p2::status::{connecting_scene, no_data_screen, NoData};
use crate::p2::ui::ScreenUi;
use crate::scene::{role_scene, Scene};
use crate::theme::Role;

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
            Role::Deploys => !(link.argocd && self.apps().have),
            Role::Face => !st.have_nodes,
            Role::GhActivity => !(link.github && self.github().have),
            Role::Weather | Role::Wind => !(link.weather && self.weather().have),
            Role::Aqi => !(link.weather && self.air().have),
            Role::Rain => !(link.rain && self.rain().have),
            Role::Sun | Role::Moon => !self.sky().have,
            Role::Iss => !self.iss().have,
            Role::Traffic => !(link.prom && self.traffic().have),
            Role::Restarts => !(link.prom && self.restarts().have),
            Role::Uptime => !(link.prom && self.uptime().have),
            Role::Fans => !(link.prom && self.fans().have),
            Role::Mains => !(link.prom && self.mains().have),
            Role::Load => !(link.prom && self.load().have),
            Role::Media => !(link.prom && self.media().have),
            Role::Ci | Role::Jellyfin => true,
        }
    }

    /// Cluster roles show the connecting scene while the API link is down.
    fn wants_connecting(&self, role: Role) -> bool {
        role.is_cluster() && !self.link().api
    }

    /// Scene for one screen at one instant; the only call the render loop makes.
    /// `ui` is that screen's widget state; it resets itself on a role change.
    pub fn scene(&self, screen: usize, now: Secs, ui: &mut ScreenUi) -> Scene {
        let state = match self.screen(screen) {
            Some(s) => s,
            None => return connecting_scene(now),
        };
        let role = state.current();
        if let Some(tr) = state.transition(now) {
            let shown = if tr.t < 0.5 { tr.from } else { tr.to };
            let mut s = self.scene_for_role(shown, now, ui);
            s.zoom = crate::screens::transition_zoom(tr.t);
            return s;
        }
        self.scene_for_role(role, now, ui)
    }

    /// Role scene without a transition (tests, goldens, calibrate).
    pub fn scene_for_role(&self, role: Role, now: Secs, ui: &mut ScreenUi) -> Scene {
        if self.wants_connecting(role) {
            return connecting_scene(now);
        }
        if role == Role::Jellyfin && !self.jellyfin_configured() {
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
        let electricity = matches!(
            role,
            Role::PowerMix | Role::Price | Role::Carbon | Role::Renewable
        );
        let why = if role.is_sky() && !self.location_present() {
            NoData::Location
        } else if (role == Role::GhActivity && !self.github_token_present())
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
    use crate::scene::Drawable;

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

    fn title_of(s: &Scene) -> Option<String> {
        s.items.iter().find_map(|d| match d {
            Drawable::CurvedText { text, .. } => Some(text.clone()),
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

    fn connecting(s: &Scene) -> bool {
        title_of(s).as_deref() == Some("CONNECTING")
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

    #[test]
    fn cycling_screen_transitions_with_iris() {
        let mut m = ready_model();
        m.set_screens(vec![vec![Role::Cpu, Role::Mem]], vec![3.0]);
        for i in 0..=100 {
            m.tick(i as f64 * 0.033);
        }
        // at 3.3 s: transition started at ~3.0, first half shows cpu shrinking
        let mut ui = ScreenUi::new();
        let s = m.scene(0, 3.15, &mut ui);
        assert!(s.zoom < 1.0 && s.zoom > 0.0);
        assert_eq!(title_of(&s).as_deref(), Some("CPU"));
        let s = m.scene(0, 3.4, &mut ui);
        assert_eq!(
            title_of(&s).as_deref(),
            Some("MEMORY"),
            "second half shows the incoming role"
        );
        // transition ends at ~3.53 s, then mem dwells; settled by 5.94 s
        for i in 101..=180 {
            m.tick(i as f64 * 0.033);
        }
        assert_eq!(m.current_role(0), Role::Mem);
        assert!(m.screen(0).unwrap().transition(5.94).is_none());
        assert_eq!(m.scene(0, 5.94, &mut ScreenUi::new()).zoom, 1.0);
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
        assert!(m.scene(0, 6.6, &mut ScreenUi::new()).zoom < 1.0);
    }

    #[test]
    fn new_roles_show_no_data_and_out_of_range_screen_connects() {
        let mut m = ready_model();
        m.set_screens(vec![vec![Role::Thermal]], vec![15.0]);
        let s = m.scene(0, 1.0, &mut ScreenUi::new());
        assert_eq!(pill_of(&s).as_deref(), Some("Waiting for data"));
        assert_eq!(s.zoom, 1.0);
        let s = m.scene(7, 1.0, &mut ScreenUi::new());
        assert!(connecting(&s));
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
        for role in [Role::PowerMix, Role::Carbon, Role::Renewable] {
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
    fn price_role_gates_on_the_prices_link() {
        let mut m = ready_model();
        m.set_token_present(true);
        m.apply(
            Event::Prices {
                date: "2026-09-07".into(),
                eur_per_kwh: vec![0.10; 96],
                avg_eur_per_kwh: 0.10,
                currency: "EUR".into(),
            },
            0.0,
        );
        m.apply(link(LinkTarget::Prices, true), 0.0);
        let live = for_role(&m, Role::Price, 1.0);
        assert_eq!(title_of(&live).as_deref(), Some("ELECTRICITY"));
        assert!(live
            .items
            .iter()
            .any(|d| matches!(d, Drawable::Run { text, .. } if text == "€/kWh")));
        m.apply(link(LinkTarget::Prices, false), 2.0);
        assert_eq!(
            pill_of(&for_role(&m, Role::Price, 2.0)).as_deref(),
            Some("Waiting for data")
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
            title_of(&for_role(&m, Role::Thermal, 1.0)).as_deref(),
            Some("THERMAL")
        );
        assert_eq!(
            title_of(&for_role(&m, Role::Storage, 1.0)).as_deref(),
            Some("STORAGE")
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
        for i in 0..=100 {
            m.tick(i as f64 * 0.033);
        }
        let s = m.scene(0, 3.4, &mut ScreenUi::new());
        assert!(s.zoom < 1.0, "mid transition");
        assert!(connecting(&s), "incoming cpu half is the connecting scene");
    }
    #[test]
    fn new_roles_wait_jellyfin_checks_config_and_exporters_can_be_missing() {
        let mut m = ready_model();
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
}
