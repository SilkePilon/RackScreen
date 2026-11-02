//! Cluster roles: cpu, mem, storage, pods, health (and its torrent mode), deploys.

use crate::anim::Secs;
use crate::event::{App, AppHealth, AppSync, Robustness};
use crate::format::{fmt_ago, fmt_capacity, fmt_eta, fmt_speed};
use crate::model::Model;
use crate::p2::chart::Card;
use crate::p2::ring::RingSpec;
use crate::p2::screen::P2Screen;
use crate::theme::p2::{BLUE, CRIT, CYAN, INDIGO, OK, TEAL, TEXT, TEXT2, TEXT3, VIOLET, WARN};
use crate::theme::Color;

pub fn cpu(model: &Model, now: Secs) -> P2Screen {
    let st = model.state();
    let hot = st.cpu_pct >= model.thresholds().hot_cpu;
    let color = if hot { WARN } else { BLUE };
    let hist = model.cpu_history();
    let mut s = P2Screen::new("CPU", format!("{:.0}", st.cpu_pct));
    s.unit = Some("%".into());
    s.ring = RingSpec::fill(st.cpu_pct / 100.0, color);
    s.pill = Some(if hot {
        ("High load".into(), WARN)
    } else if st.nodes_total > 0 {
        (format!("{} nodes · avg", st.nodes_total), TEXT2)
    } else {
        ("cluster avg".into(), TEXT2)
    });
    s.card = Some(Card::Spark {
        series: hist.series(),
        prev: hist.prev(),
        scroll: hist.scroll(now),
        min: 0.0,
        max: 100.0,
        now_idx: None,
        color,
        caption: "last 60 s".into(),
    });
    s
}

pub fn mem(model: &Model, now: Secs) -> P2Screen {
    let st = model.state();
    let hist = model.mem_history();
    let mut s = P2Screen::new("MEMORY", format!("{:.0}", st.mem_pct));
    s.unit = Some("%".into());
    s.ring = RingSpec::fill(st.mem_pct / 100.0, VIOLET);
    s.pill = Some((
        format!("{:.1} of {:.0} GB", st.mem_used_gb, st.mem_total_gb),
        TEXT2,
    ));
    s.card = Some(Card::Spark {
        series: hist.series(),
        prev: hist.prev(),
        scroll: hist.scroll(now),
        min: 50.0,
        max: 80.0,
        now_idx: None,
        color: VIOLET,
        caption: "last 90 s".into(),
    });
    s
}

/// Teal, darker for each further volume.
pub fn teal_shade(i: usize, n: usize) -> Color {
    let t = if n <= 1 {
        0.0
    } else {
        i as f32 / (n as f32 - 1.0) * 0.9
    };
    TEAL.mix(Color::hex(0x2F6866), t)
}

pub fn storage(model: &Model, _now: Secs) -> P2Screen {
    let st = model.state();
    let cap = st.storage_capacity.max(1) as f32;
    let used_pct = if st.storage_capacity > 0 {
        st.storage_used as f32 / cap * 100.0
    } else {
        0.0
    };
    let degraded: Vec<&str> = st
        .volumes
        .iter()
        .filter(|(_, r)| matches!(r, Robustness::Degraded | Robustness::Faulted))
        .map(|(n, _)| n.as_str())
        .collect();
    let mut sized: Vec<(&str, u64)> = st
        .volume_bytes
        .iter()
        .map(|(n, b)| (n.as_str(), *b))
        .collect();
    sized.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    let n = sized.len();
    let parts: Vec<(f32, Color)> = if sized.is_empty() {
        vec![(used_pct / 100.0, TEAL)]
    } else {
        sized
            .iter()
            .enumerate()
            .map(|(i, (name, bytes))| {
                let c = if degraded.contains(name) {
                    CRIT
                } else {
                    teal_shade(i, n)
                };
                (*bytes as f32 / cap, c)
            })
            .collect()
    };
    let (of, free) = fmt_capacity(st.storage_used, st.storage_capacity);
    let mut s = P2Screen::new("STORAGE", format!("{used_pct:.0}"));
    s.unit = Some("%".into());
    s.ring = RingSpec::parts(parts.clone());
    s.pill = Some(match degraded.first() {
        Some(v) => (format!("{v} degraded"), CRIT),
        None => (of, TEXT2),
    });
    s.card = Some(Card::StackBar {
        parts,
        caption: match degraded.first() {
            Some(v) => format!("{v} degraded"),
            None => free,
        },
    });
    s
}

pub fn pods(model: &Model, _now: Secs) -> P2Screen {
    let st = model.state();
    let total = st.pods_total.max(1) as f32;
    let mut s = P2Screen::new("PODS", st.pods_running.to_string());
    s.ring = RingSpec::parts(vec![
        (st.pods_running as f32 / total, OK),
        (st.pods_pending as f32 / total, WARN),
        (st.pods_failed as f32 / total, CRIT),
    ]);
    s.pill = Some(("running".into(), TEXT2));
    s.card = Some(Card::Stats(vec![
        (
            st.pods_pending.to_string(),
            "pending".into(),
            if st.pods_pending > 0 { WARN } else { TEXT3 },
        ),
        (
            st.pods_failed.to_string(),
            "failed".into(),
            if st.pods_failed > 0 { CRIT } else { TEXT3 },
        ),
    ]));
    s
}

pub fn health(model: &Model, now: Secs) -> P2Screen {
    if model.torrent_mode() {
        return torrent(model, now);
    }
    let st = model.state();
    let (ready, total) = (st.nodes_ready as usize, st.nodes_total as usize);
    let not_ready = total.saturating_sub(ready);
    let mut s = P2Screen::new("CLUSTER", format!("{ready}/{total}"));
    if format!("{ready}/{total}").len() >= 5 {
        s.hero_scale = 0.82;
    }
    s.ring = RingSpec::discrete(
        (0..total)
            .map(|i| if i < ready { OK } else { CRIT })
            .collect(),
    );
    s.pill = Some(match st.nodes_not_ready.first() {
        Some(n) => (format!("{n} not ready"), CRIT),
        None if not_ready > 0 => ("node not ready".into(), CRIT),
        None => ("All nodes ready".into(), OK),
    });
    let alerts = st.alerts.len();
    s.card = Some(Card::Stats(vec![
        (
            alerts.to_string(),
            "alerts".into(),
            if alerts > 0 { CRIT } else { TEXT3 },
        ),
        (
            not_ready.to_string(),
            "not ready".into(),
            if not_ready > 0 { CRIT } else { TEXT3 },
        ),
    ]));
    s.live = true;
    s
}

/// HEALTH while qBittorrent is busy: the most complete download.
pub fn torrent(model: &Model, _now: Secs) -> P2Screen {
    let list = model.torrent_badge_list();
    let top = list.iter().max_by(|a, b| a.progress.total_cmp(&b.progress));
    let (name, progress) = top
        .map(|t| (t.name.clone(), t.progress))
        .unwrap_or_default();
    let mut s = P2Screen::new("DOWNLOADS", format!("{progress:.0}"));
    s.unit = Some("%".into());
    s.ring = RingSpec::fill(progress / 100.0, CYAN);
    s.pill = Some((name, CYAN));
    let speed: i64 = list.iter().map(|t| t.speed_bps).sum();
    let eta = list
        .iter()
        .map(|t| t.eta_secs)
        .filter(|e| (0..864_000).contains(e))
        .min()
        .unwrap_or(-1);
    s.card = Some(Card::Stats(vec![
        (fmt_speed(speed), "speed".into(), TEXT),
        (fmt_eta(eta), "ETA".into(), TEXT),
    ]));
    s
}

/// Segment colour for an Argo application.
pub fn app_color(a: &App) -> Color {
    if matches!(a.health, AppHealth::Degraded | AppHealth::Missing) {
        CRIT
    } else if matches!(a.health, AppHealth::Suspended | AppHealth::Unknown)
        || a.sync == AppSync::Unknown
    {
        TEXT3
    } else if a.health == AppHealth::Progressing || a.sync == AppSync::OutOfSync || a.operating {
        BLUE
    } else {
        OK
    }
}

pub fn deploys(model: &Model, now: Secs) -> P2Screen {
    let apps = &model.apps().apps;
    let log = model.deploys_log();
    let today: u32 = log.hours.iter().sum();
    let mut s = P2Screen::new("DEPLOYS TODAY", today.to_string());
    s.ring = RingSpec::discrete(apps.iter().map(app_color).collect());
    s.pill = Some(match &log.last {
        Some((name, at)) if now - at < 4.0 => (format!("{name} · just now"), INDIGO),
        Some((name, at)) => (format!("{name} · {}", fmt_ago(now - at)), TEXT2),
        None if today > 0 => ("since midnight".into(), TEXT2),
        None => ("no syncs yet".into(), TEXT2),
    });
    s.card = Some(Card::Bars {
        values: log.hours.iter().map(|h| *h as f32).collect(),
        colors: vec![INDIGO; 24],
        now: model.local_hour(),
        past_alpha: 0.45,
        caption: "per hour".into(),
    });
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{App, AppHealth, AppSync, Event, LinkTarget, Robustness, Torrent};
    use crate::model::Thresholds;
    use crate::p2::ring::RingKind;
    use crate::theme::p2::{BLUE, CRIT, INDIGO, OK, TEAL, TEXT2, TEXT3, VIOLET, WARN};

    fn base() -> Model {
        let mut m = Model::new(Thresholds::default());
        m.apply(
            Event::Metrics {
                cpu_pct: 42.4,
                mem_pct: 67.0,
                mem_used_gb: 10.7,
                mem_total_gb: 16.0,
                hot_cpu: None,
                hot_mem: None,
            },
            0.0,
        );
        m.apply(
            Event::NodeSnapshot {
                ready: 7,
                total: 7,
                not_ready: vec![],
            },
            0.0,
        );
        m
    }

    fn pill(s: &P2Screen) -> (String, Color) {
        s.pill.clone().expect("pill")
    }

    #[test]
    fn cpu_and_mem() {
        let m = base();
        let s = cpu(&m, 1.0);
        assert_eq!(
            (s.title.as_str(), s.hero.as_str(), s.unit.as_deref()),
            ("CPU", "42", Some("%"))
        );
        assert!(
            matches!(s.ring.kind, RingKind::Fill { frac, color } if (frac - 0.424).abs() < 1e-4 && color == BLUE)
        );
        assert_eq!(pill(&s), ("7 nodes · avg".into(), TEXT2));
        assert!(
            matches!(&s.card, Some(Card::Spark { series, max, caption, .. }) if series == &vec![42.4] && *max == 100.0 && caption == "last 60 s")
        );
        let mut hot = base();
        hot.apply(
            Event::Metrics {
                cpu_pct: 93.0,
                mem_pct: 67.0,
                mem_used_gb: 10.7,
                mem_total_gb: 16.0,
                hot_cpu: None,
                hot_mem: None,
            },
            1.0,
        );
        let s = cpu(&hot, 1.0);
        assert_eq!(pill(&s), ("High load".into(), WARN));
        assert!(matches!(s.ring.kind, RingKind::Fill { color, .. } if color == WARN));
        let s = mem(&m, 1.0);
        assert_eq!((s.title.as_str(), s.hero.as_str()), ("MEMORY", "67"));
        assert_eq!(pill(&s), ("10.7 of 16 GB".into(), TEXT2));
        assert!(matches!(s.ring.kind, RingKind::Fill { color, .. } if color == VIOLET));
        assert!(
            matches!(&s.card, Some(Card::Spark { min, max, caption, .. }) if *min == 50.0 && *max == 80.0 && caption == "last 90 s")
        );
    }

    #[test]
    fn storage_parts_follow_volume_sizes_and_flag_degraded_volumes() {
        let mut m = base();
        let tb = 1u64 << 40;
        m.apply(
            Event::Storage {
                volumes: vec![
                    ("graf".into(), Robustness::Healthy),
                    ("prom".into(), Robustness::Healthy),
                    ("qbit".into(), Robustness::Healthy),
                ],
                used_bytes: tb + tb / 5,
                capacity_bytes: 3 * tb + tb / 10,
            },
            0.0,
        );
        m.apply(
            Event::VolumeSizes(vec![
                ("graf".into(), tb / 4),
                ("prom".into(), tb / 2),
                ("qbit".into(), 2 * tb / 5),
            ]),
            0.0,
        );
        let s = storage(&m, 1.0);
        assert_eq!((s.hero.as_str(), s.unit.as_deref()), ("39", Some("%")));
        assert_eq!(pill(&s), ("1.2 of 3.1 TB".into(), TEXT2));
        let parts = match &s.ring.kind {
            RingKind::Parts(p) => p.clone(),
            _ => panic!("parts"),
        };
        assert_eq!(parts.len(), 3);
        assert!((parts[0].0 - 0.5 / 3.1).abs() < 1e-3, "largest first: prom");
        assert_eq!(parts[0].1, TEAL);
        assert_ne!(parts[1].1, parts[0].1, "each volume its own shade");
        assert!(
            matches!(&s.card, Some(Card::StackBar { caption, .. }) if caption == "1.9 TB free")
        );
        m.apply(
            Event::Storage {
                volumes: vec![
                    ("graf".into(), Robustness::Healthy),
                    ("prom".into(), Robustness::Healthy),
                    ("qbit".into(), Robustness::Degraded),
                ],
                used_bytes: tb + tb / 5,
                capacity_bytes: 3 * tb + tb / 10,
            },
            1.0,
        );
        let s = storage(&m, 2.0);
        assert_eq!(pill(&s), ("qbit degraded".into(), CRIT));
        let parts = match &s.ring.kind {
            RingKind::Parts(p) => p.clone(),
            _ => panic!("parts"),
        };
        assert_eq!(parts[1].1, CRIT, "qbit is the second largest");
        assert!(
            matches!(&s.card, Some(Card::StackBar { caption, .. }) if caption == "qbit degraded")
        );
        let mut bare = base();
        bare.apply(
            Event::Storage {
                volumes: vec![("a".into(), Robustness::Healthy)],
                used_bytes: 64 << 30,
                capacity_bytes: 128 << 30,
            },
            0.0,
        );
        assert!(
            matches!(&storage(&bare, 0.0).ring.kind, RingKind::Parts(p) if p.len() == 1 && (p[0].0 - 0.5).abs() < 1e-4),
            "no sizes: one used part"
        );
    }

    #[test]
    fn pods_health_and_torrent_mode() {
        let mut m = base();
        m.apply(
            Event::PodSnapshot {
                running: 38,
                pending: 2,
                failed: 1,
                total: 41,
            },
            0.0,
        );
        let s = pods(&m, 0.0);
        assert_eq!(s.hero, "38");
        assert!(
            matches!(&s.ring.kind, RingKind::Parts(p) if p.len() == 3 && p[0].1 == OK && p[1].1 == WARN && p[2].1 == CRIT && (p[0].0 - 38.0 / 41.0).abs() < 1e-5)
        );
        assert_eq!(pill(&s), ("running".into(), TEXT2));
        assert!(
            matches!(&s.card, Some(Card::Stats(items)) if items[0] == ("2".to_string(), "pending".to_string(), WARN) && items[1].2 == CRIT)
        );
        let h = health(&m, 0.0);
        assert_eq!(
            (h.title.as_str(), h.hero.as_str(), h.live),
            ("CLUSTER", "7/7", true)
        );
        assert!(
            matches!(&h.ring.kind, RingKind::Discrete(c) if c.len() == 7 && c.iter().all(|c| *c == OK))
        );
        assert_eq!(pill(&h), ("All nodes ready".into(), OK));
        assert!(
            matches!(&h.card, Some(Card::Stats(items)) if items[0].1 == "alerts" && items[0].2 == TEXT3 && items[1] == ("0".to_string(), "not ready".to_string(), TEXT3))
        );
        m.apply(
            Event::NodeSnapshot {
                ready: 6,
                total: 7,
                not_ready: vec!["pi3".into()],
            },
            1.0,
        );
        m.apply(
            Event::AlertSnapshot {
                firing: vec!["Down".into()],
            },
            1.0,
        );
        let h = health(&m, 1.0);
        assert_eq!(h.hero, "6/7");
        assert_eq!(pill(&h), ("pi3 not ready".into(), CRIT));
        assert!(matches!(&h.ring.kind, RingKind::Discrete(c) if c[5] == OK && c[6] == CRIT));
        assert!(
            matches!(&h.card, Some(Card::Stats(items)) if items[0] == ("1".to_string(), "alerts".to_string(), CRIT) && items[1].0 == "1")
        );
        let mut big = base();
        big.apply(
            Event::NodeSnapshot {
                ready: 12,
                total: 12,
                not_ready: vec![],
            },
            0.0,
        );
        assert_eq!(health(&big, 0.0).hero_scale, 0.82, "five characters shrink");
        let mut t = base();
        t.apply(
            Event::Link {
                target: LinkTarget::QBittorrent,
                up: true,
            },
            0.0,
        );
        t.apply(
            Event::Torrents(vec![
                Torrent {
                    name: "ubuntu-24.04.2-live-server-amd64.iso".into(),
                    progress: 78.0,
                    eta_secs: 900,
                    speed_bps: 9_000_000,
                },
                Torrent {
                    name: "b".into(),
                    progress: 41.0,
                    eta_secs: 3000,
                    speed_bps: 3_000_000,
                },
            ]),
            0.0,
        );
        t.tick(1.0);
        let s = health(&t, 1.0);
        assert_eq!((s.title.as_str(), s.hero.as_str()), ("DOWNLOADS", "78"));
        assert!(
            matches!(s.ring.kind, RingKind::Fill { frac, color } if (frac - 0.78).abs() < 1e-5 && color == CYAN)
        );
        assert_eq!(pill(&s).0, "ubuntu-24.04.2-live-server-amd64.iso");
        assert!(
            matches!(&s.card, Some(Card::Stats(items)) if items[0] == ("11.4M".to_string(), "speed".to_string(), TEXT) && items[1] == ("15m".to_string(), "ETA".to_string(), TEXT))
        );
    }

    #[test]
    fn deploys_count_today_and_name_the_last_sync() {
        let mut m = base();
        m.set_unix_now(1_788_782_400);
        m.set_utc_offset_secs(7200);
        m.apply(
            Event::Link {
                target: LinkTarget::ArgoCd,
                up: true,
            },
            0.0,
        );
        let app = |name: &str, sync, health, operating| App {
            name: name.into(),
            sync,
            health,
            operating,
        };
        m.apply(
            Event::Apps(vec![
                app("a", AppSync::Synced, AppHealth::Healthy, false),
                app("b", AppSync::OutOfSync, AppHealth::Progressing, true),
                app("c", AppSync::Synced, AppHealth::Degraded, false),
                app("d", AppSync::Unknown, AppHealth::Unknown, false),
            ]),
            0.0,
        );
        let s = deploys(&m, 0.0);
        assert_eq!((s.title.as_str(), s.hero.as_str()), ("DEPLOYS TODAY", "0"));
        assert!(matches!(&s.ring.kind, RingKind::Discrete(c) if c == &vec![OK, BLUE, CRIT, TEXT3]));
        assert_eq!(pill(&s), ("no syncs yet".into(), TEXT2));
        assert!(
            matches!(&s.card, Some(Card::Bars { values, now, past_alpha, caption, .. }) if values.len() == 24 && *now == 14 && *past_alpha == 0.45 && caption == "per hour")
        );
        m.apply(
            Event::AppSynced {
                name: "grafana".into(),
            },
            10.0,
        );
        let s = deploys(&m, 12.0);
        assert_eq!(s.hero, "1");
        assert_eq!(pill(&s), ("grafana · just now".into(), INDIGO));
        let s = deploys(&m, 140.0);
        assert_eq!(pill(&s), ("grafana · 2 m ago".into(), TEXT2));
        assert!(matches!(&s.card, Some(Card::Bars { values, .. }) if values[14] == 1.0));
    }
}
