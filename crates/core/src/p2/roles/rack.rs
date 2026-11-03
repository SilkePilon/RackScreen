//! The six hardware roles read from Prometheus: traffic, restarts, uptime,
//! fans, mains and load.

use crate::anim::Secs;
use crate::format::{fmt_ago, fmt_days};
use crate::model::Model;
use crate::p2::chart::Card;
use crate::p2::ring::RingSpec;
use crate::p2::roles::short_node;
use crate::p2::screen::P2Screen;
use crate::theme::p2::{CRIT, CYAN, INDIGO, OK, TEXT, TEXT2, TEXT3, TRACK, WARN};
use crate::theme::Color;

/// The three busiest services on the traffic ring, busiest first.
pub const INDIGO_SHADES: [Color; 3] = [INDIGO, Color::hex(0x6E7BD0), Color::hex(0x525C9E)];
/// Fan power (pwm duty) is amber above this percent.
pub const FAN_WARN_PCT: f32 = 85.0;
/// Mains is "stable" inside this window.
pub const MAINS_OK_V: (f32, f32) = (220.0, 240.0);

pub fn traffic(model: &Model, now: Secs) -> P2Screen {
    let t = model.traffic();
    let hist = model.traffic_history();
    let mut s = P2Screen::new("WEB TRAFFIC", format!("{:.1}", t.req_per_sec));
    s.unit = Some("req/s".into());
    s.unit_below = true;
    let total = t.service_rps.max(1e-6);
    s.ring = RingSpec::parts(
        t.top
            .iter()
            .enumerate()
            .map(|(i, (_, r))| (r / total, INDIGO_SHADES[i.min(2)]))
            .collect(),
    );
    let busiest = t.top.first().map(|(n, _)| n.as_str()).unwrap_or("quiet");
    let err_pct = t.error_share * 100.0;
    let color = if t.error_share >= 0.05 {
        CRIT
    } else if t.error_share >= 0.01 {
        WARN
    } else {
        TEXT2
    };
    s.pill = Some((format!("{busiest} · {err_pct:.0} % errors"), color));
    let series = hist.series();
    let max = (series.iter().cloned().fold(0.0_f32, f32::max) * 1.1).max(1.0);
    s.card = Some(Card::Spark {
        series,
        prev: hist.prev(),
        scroll: hist.scroll(now),
        min: 0.0,
        max,
        now_idx: None,
        color: INDIGO,
        caption: "last 90 s".into(),
    });
    s
}

/// Ring segment colour for one hour's restarts: absolute thresholds.
pub fn restart_color(n: f32) -> Color {
    if n >= 6.0 {
        CRIT
    } else if n >= 2.0 {
        WARN
    } else if n >= 1.0 {
        OK.with_alpha(0.8)
    } else {
        TRACK
    }
}

pub fn restarts(model: &Model, _now: Secs) -> P2Screen {
    let r = model.restarts();
    let values: Vec<f32> = model
        .restarts_hourly()
        .last_24(model.unix_hour())
        .iter()
        .map(|v| v.unwrap_or(0.0))
        .collect();
    let mut s = P2Screen::new("RESTARTS · 24H", r.total_24h.to_string());
    s.ring = RingSpec::discrete(values.iter().map(|v| restart_color(*v)).collect());
    s.pill = Some(match &r.worst {
        Some((pod, n)) if r.total_24h > 0 => {
            (format!("{pod} ×{n}"), if *n >= 6 { CRIT } else { TEXT2 })
        }
        _ => ("No restarts".into(), OK),
    });
    s.card = Some(Card::Bars {
        colors: values
            .iter()
            .map(|v| match restart_color(*v) {
                c if c == TRACK => TEXT3,
                c if c == OK.with_alpha(0.8) => OK,
                c => c,
            })
            .collect(),
        values,
        now: 23,
        past_alpha: 0.9,
        caption: "per hour".into(),
    });
    s
}

pub fn uptime(model: &Model, _now: Secs) -> P2Screen {
    let u = model.uptime();
    let max = u.nodes.iter().map(|n| n.1).fold(0.0_f64, f64::max);
    let mut s = P2Screen::new("UPTIME", fmt_days(max));
    s.unit = Some("d".into());
    let color = |secs: f64| if secs < 86_400.0 { WARN } else { OK };
    s.ring = RingSpec::discrete(u.nodes.iter().map(|(_, secs)| color(*secs)).collect());
    let recent = u
        .nodes
        .iter()
        .filter(|(_, secs)| *secs < 86_400.0)
        .min_by(|a, b| a.1.total_cmp(&b.1));
    s.pill = Some(match recent {
        Some((name, secs)) => (format!("{name} rebooted {}", fmt_ago(*secs)), WARN),
        None => {
            let min = u.nodes.iter().map(|n| n.1).fold(f64::MAX, f64::min);
            (format!("All up {} d+", fmt_days(min.min(max))), OK)
        }
    });
    s.card = Some(Card::NodeBars {
        values: u
            .nodes
            .iter()
            .map(|(_, secs)| (*secs / 86_400.0) as f32)
            .collect(),
        min: 0.0,
        max: ((max / 86_400.0) as f32).max(1.0),
        labels: u.nodes.iter().map(|(n, _)| short_node(n)).collect(),
        colors: u.nodes.iter().map(|(_, secs)| color(*secs)).collect(),
        highlight: None,
    });
    s
}

pub fn fans(model: &Model, _now: Secs) -> P2Screen {
    let f = model.fans();
    let busiest = f
        .fans
        .iter()
        .enumerate()
        .max_by(|a, b| a.1 .1.total_cmp(&b.1 .1));
    let (idx, name, max) = match busiest {
        Some((i, (n, pct))) => (Some(i), n.as_str(), *pct),
        None => (None, "", 0.0),
    };
    let rounded = |pct: f32| pct.round() as i32;
    let hero = rounded(max);
    let mut s = P2Screen::new("FANS", hero.to_string());
    s.unit = Some("%".into());
    let color = |pct: f32| if pct > FAN_WARN_PCT { WARN } else { CYAN };
    s.ring = RingSpec::fill((max / 100.0).clamp(0.0, 1.0), color(max));
    let all_same = f.fans.iter().all(|(_, p)| rounded(*p) == hero);
    let pill = if all_same && !f.fans.is_empty() {
        format!("All at {hero} %")
    } else {
        format!("{name} busiest")
    };
    s.pill = Some((pill, TEXT2));
    s.card = Some(Card::NodeBars {
        values: f.fans.iter().map(|(_, pct)| *pct).collect(),
        min: 0.0,
        max: 100.0,
        labels: f.fans.iter().map(|(n, _)| short_node(n)).collect(),
        colors: f.fans.iter().map(|(_, pct)| color(*pct)).collect(),
        highlight: idx,
    });
    s
}

/// `Stable · UPS load 7 %` inside 220–240 V, else low or high voltage.
pub fn mains_pill(volts: f32, ups_load_pct: f32) -> (String, Color) {
    if volts < MAINS_OK_V.0 {
        ("Low voltage".into(), WARN)
    } else if volts > MAINS_OK_V.1 {
        ("High voltage".into(), WARN)
    } else {
        (format!("Stable · UPS load {ups_load_pct:.0} %"), OK)
    }
}

pub fn mains(model: &Model, _now: Secs) -> P2Screen {
    let m = model.mains();
    let mut s = P2Screen::new("MAINS", format!("{:.0}", m.volts));
    s.unit = Some("V".into());
    s.ring = RingSpec::bands(
        vec![CRIT, WARN, OK, OK, WARN, CRIT],
        (m.volts - 200.0) / 60.0,
    );
    s.ends = Some(("200".into(), "260".into()));
    let (text, color) = mains_pill(m.volts, m.ups_load_pct);
    s.pill = Some((text, color));
    let series = model.mains_hourly().values();
    let lo = series.iter().cloned().fold(f32::MAX, f32::min);
    let hi = series.iter().cloned().fold(f32::MIN, f32::max);
    let pad = ((hi - lo) * 0.2).max(3.0);
    s.card = Some(Card::Spark {
        now_idx: Some(series.len().saturating_sub(1)),
        series,
        prev: None,
        scroll: 1.0,
        min: lo - pad,
        max: hi + pad,
        color,
        caption: "24 h".into(),
    });
    s
}

/// Segment colour for a node's 1-minute load per core.
pub fn load_color(per_core: f32) -> Color {
    if per_core >= 0.9 {
        CRIT
    } else if per_core >= 0.7 {
        WARN
    } else {
        OK
    }
}

pub fn load(model: &Model, _now: Secs) -> P2Screen {
    let l = model.load();
    let busiest = l.nodes.iter().max_by(|a, b| a.load1.total_cmp(&b.load1));
    let max = busiest.map(|n| n.load1).unwrap_or(0.0);
    let mut s = P2Screen::new("LOAD", format!("{max:.2}"));
    if max >= 10.0 {
        s.hero_scale = 0.82;
    }
    s.ring = RingSpec::discrete(
        l.nodes
            .iter()
            .map(|n| load_color(n.load1 / n.cores.max(1) as f32))
            .collect(),
    );
    s.pill = Some((
        format!("{} busiest", busiest.map(|n| n.node.as_str()).unwrap_or("")),
        TEXT2,
    ));
    let n = l.nodes.len().max(1) as f32;
    let mean = |f: fn(&crate::event::NodeLoad) -> f32| l.nodes.iter().map(f).sum::<f32>() / n;
    s.card = Some(Card::Stats(vec![
        (format!("{:.2}", mean(|x| x.load1)), "1 min".into(), TEXT),
        (format!("{:.2}", mean(|x| x.load5)), "5 min".into(), TEXT),
        (format!("{:.2}", mean(|x| x.load15)), "15 min".into(), TEXT),
    ]));
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{Event, NodeLoad};
    use crate::model::Thresholds;
    use crate::p2::chart::Card;
    use crate::p2::ring::RingKind;
    use crate::theme::p2::{CRIT, CYAN, INDIGO, OK, TEXT, TEXT2, TEXT3, TRACK, WARN};

    fn pill(s: &P2Screen) -> (String, Color) {
        s.pill.clone().expect("pill")
    }

    #[test]
    fn traffic_shares_the_top_three_and_flags_errors() {
        let mut m = Model::new(Thresholds::default());
        m.apply(
            Event::TrafficHistory {
                req_per_sec: vec![3.0; 31],
            },
            0.0,
        );
        m.apply(
            Event::Traffic {
                req_per_sec: 4.2,
                top: vec![
                    ("n8n".into(), 2.0),
                    ("grafana".into(), 1.0),
                    ("pihole".into(), 0.5),
                ],
                service_rps: 4.0,
                error_share: 0.0,
            },
            1.0,
        );
        let s = traffic(&m, 1.0);
        assert_eq!(
            (
                s.title.as_str(),
                s.hero.as_str(),
                s.unit.as_deref(),
                s.unit_below
            ),
            ("WEB TRAFFIC", "4.2", Some("req/s"), true)
        );
        match &s.ring.kind {
            RingKind::Parts(p) => {
                assert_eq!(p.len(), 3);
                assert!((p[0].0 - 0.5).abs() < 1e-5 && p[0].1 == INDIGO);
                assert!((p[1].0 - 0.25).abs() < 1e-5 && p[1].1 == INDIGO_SHADES[1]);
                assert!((p[2].0 - 0.125).abs() < 1e-5 && p[2].1 == INDIGO_SHADES[2]);
            }
            _ => panic!("parts"),
        }
        assert_eq!(pill(&s), ("n8n · 0 % errors".into(), TEXT2));
        assert!(
            matches!(&s.card, Some(Card::Spark { series, min, max, color, caption, .. }) if series.len() == 31 && series[30] == 4.2 && *min == 0.0 && (*max - 4.62).abs() < 1e-3 && *color == INDIGO && caption == "last 90 s")
        );
        m.apply(
            Event::Traffic {
                req_per_sec: 4.2,
                top: vec![("n8n".into(), 2.0)],
                service_rps: 4.0,
                error_share: 0.012,
            },
            2.0,
        );
        assert_eq!(pill(&traffic(&m, 2.0)), ("n8n · 1 % errors".into(), WARN));
        m.apply(
            Event::Traffic {
                req_per_sec: 0.0,
                top: vec![],
                service_rps: 0.0,
                error_share: 0.07,
            },
            3.0,
        );
        let quiet = traffic(&m, 3.0);
        assert_eq!(pill(&quiet), ("quiet · 7 % errors".into(), CRIT));
        assert!(matches!(&quiet.ring.kind, RingKind::Parts(p) if p.is_empty()));
    }

    #[test]
    fn restarts_colour_hours_by_absolute_thresholds() {
        let mut m = Model::new(Thresholds::default());
        m.set_unix_now(3600 * 1000 + 10);
        let mut hourly: Vec<(i64, f32)> = (0..24).map(|i| (1000 - 23 + i, 0.0)).collect();
        hourly[5].1 = 1.0;
        hourly[9].1 = 3.0;
        hourly[20].1 = 41.0;
        m.apply(Event::RestartsHistory { hourly }, 0.0);
        m.apply(
            Event::Restarts {
                total_24h: 45,
                worst: Some(("n8n-worker".into(), 41)),
            },
            0.0,
        );
        let s = restarts(&m, 0.0);
        assert_eq!(
            (s.title.as_str(), s.hero.as_str()),
            ("RESTARTS · 24H", "45")
        );
        match &s.ring.kind {
            RingKind::Discrete(c) => {
                assert_eq!(c.len(), 24);
                assert_eq!(c[0], TRACK);
                assert_eq!(c[5], OK.with_alpha(0.8));
                assert_eq!(c[9], WARN);
                assert_eq!(c[20], CRIT);
                assert_eq!(c[23], TRACK, "the current hour is quiet");
            }
            _ => panic!("discrete"),
        }
        assert_eq!(pill(&s), ("n8n-worker ×41".into(), CRIT));
        match &s.card {
            Some(Card::Bars {
                values,
                colors,
                now,
                past_alpha,
                caption,
            }) => {
                assert_eq!(values.len(), 24);
                assert_eq!(values[20], 41.0);
                assert_eq!(colors[0], TEXT3);
                assert_eq!(colors[5], OK);
                assert_eq!(colors[9], WARN);
                assert_eq!(colors[20], CRIT);
                assert_eq!((*now, *past_alpha, caption.as_str()), (23, 0.9, "per hour"));
            }
            _ => panic!("bars"),
        }
        m.apply(
            Event::Restarts {
                total_24h: 2,
                worst: Some(("x".into(), 2)),
            },
            1.0,
        );
        assert_eq!(pill(&restarts(&m, 1.0)), ("x ×2".into(), TEXT2));
        m.apply(
            Event::Restarts {
                total_24h: 0,
                worst: None,
            },
            2.0,
        );
        assert_eq!(pill(&restarts(&m, 2.0)), ("No restarts".into(), OK));
        assert_eq!(restart_color(0.0), TRACK);
        assert_eq!(restart_color(5.9), WARN);
        assert_eq!(restart_color(6.0), CRIT);
    }

    #[test]
    fn uptime_names_the_latest_reboot_or_the_shortest_uptime() {
        let mut m = Model::new(Thresholds::default());
        m.apply(
            Event::Uptime(vec![
                ("hp-elitedesk-800-g5-i7".into(), 82.1 * 86_400.0),
                ("raspberrypi-5-8gb-2".into(), 9.0 * 3600.0),
                ("raspberrypi-5-8gb-3".into(), 3.4 * 86_400.0),
            ]),
            0.0,
        );
        let s = uptime(&m, 0.0);
        assert_eq!(
            (
                s.title.as_str(),
                s.hero.as_str(),
                s.unit.as_deref(),
                s.unit_below
            ),
            ("UPTIME", "82", Some("d"), false)
        );
        assert!(matches!(&s.ring.kind, RingKind::Discrete(c) if c == &vec![OK, WARN, OK]));
        assert_eq!(
            pill(&s),
            ("raspberrypi-5-8gb-2 rebooted 9 h ago".into(), WARN)
        );
        match &s.card {
            Some(Card::NodeBars {
                values,
                min,
                max,
                labels,
                colors,
                highlight,
            }) => {
                assert!((values[0] - 82.1).abs() < 1e-3 && (values[1] - 0.375).abs() < 1e-3);
                assert_eq!((*min, *max, *highlight), (0.0, 82.1, None));
                assert_eq!(labels, &vec!["i7".to_string(), "r2".into(), "r3".into()]);
                assert_eq!(colors, &vec![OK, WARN, OK]);
            }
            _ => panic!("node bars"),
        }
        m.apply(
            Event::Uptime(vec![
                ("a".into(), 82.1 * 86_400.0),
                ("b".into(), 3.4 * 86_400.0),
            ]),
            1.0,
        );
        assert_eq!(pill(&uptime(&m, 1.0)), ("All up 3 d+".into(), OK));
    }

    #[test]
    fn fans_show_the_busiest_fan_power_in_percent() {
        let mut m = Model::new(Thresholds::default());
        m.apply(
            Event::Fans(vec![
                ("raspberrypi-5-16gb-1".into(), 45.1),
                ("raspberrypi-5-8gb-1".into(), 40.0),
                ("raspberrypi-5-8gb-3".into(), 58.6),
            ]),
            0.0,
        );
        let s = fans(&m, 0.0);
        assert_eq!(
            (
                s.title.as_str(),
                s.hero.as_str(),
                s.unit.as_deref(),
                s.unit_below
            ),
            ("FANS", "59", Some("%"), false)
        );
        assert!(
            matches!(s.ring.kind, RingKind::Fill { frac, color } if (frac - 0.586).abs() < 1e-5 && color == CYAN)
        );
        assert_eq!(pill(&s), ("raspberrypi-5-8gb-3 busiest".into(), TEXT2));
        match &s.card {
            Some(Card::NodeBars {
                values,
                min,
                max,
                labels,
                colors,
                highlight,
            }) => {
                assert_eq!(values, &vec![45.1, 40.0, 58.6]);
                assert_eq!((*min, *max, *highlight), (0.0, 100.0, Some(2)));
                assert_eq!(labels, &vec!["r1".to_string(), "r1".into(), "r3".into()]);
                assert_eq!(colors, &vec![CYAN; 3]);
            }
            _ => panic!("node bars"),
        }
    }

    #[test]
    fn fans_warn_above_85_percent() {
        let mut m = Model::new(Thresholds::default());
        m.apply(
            Event::Fans(vec![("a".into(), 92.0), ("b".into(), 49.0)]),
            0.0,
        );
        let s = fans(&m, 0.0);
        assert_eq!(s.hero, "92");
        assert!(matches!(s.ring.kind, RingKind::Fill { color, .. } if color == WARN));
        match &s.card {
            Some(Card::NodeBars { colors, .. }) => assert_eq!(colors, &vec![WARN, CYAN]),
            _ => panic!("node bars"),
        }
        m.apply(Event::Fans(vec![("a".into(), FAN_WARN_PCT)]), 1.0);
        let at = fans(&m, 1.0);
        assert!(matches!(at.ring.kind, RingKind::Fill { color, .. } if color == CYAN));
    }

    #[test]
    fn fans_pill_says_all_at_when_every_node_rounds_the_same() {
        let mut m = Model::new(Thresholds::default());
        m.apply(
            Event::Fans(vec![
                ("a".into(), 49.02),
                ("b".into(), 49.02),
                ("c".into(), 48.8),
            ]),
            0.0,
        );
        let s = fans(&m, 0.0);
        assert_eq!(s.hero, "49");
        assert_eq!(pill(&s), ("All at 49 %".into(), TEXT2));
    }

    #[test]
    fn mains_bands_the_voltage_and_charts_the_day() {
        let mut m = Model::new(Thresholds::default());
        m.set_unix_now(3600 * 1000 + 10);
        m.apply(
            Event::MainsHistory {
                hourly: (0..24)
                    .map(|i| (1000 - 23 + i, 232.0 + (i % 3) as f32))
                    .collect(),
            },
            0.0,
        );
        m.apply(
            Event::Mains {
                volts: 234.0,
                ups_load_pct: 7.0,
            },
            0.0,
        );
        let s = mains(&m, 0.0);
        assert_eq!(
            (
                s.title.as_str(),
                s.hero.as_str(),
                s.unit.as_deref(),
                s.unit_below
            ),
            ("MAINS", "234", Some("V"), false)
        );
        assert!(
            matches!(&s.ring.kind, RingKind::Bands { colors, frac } if colors == &vec![CRIT, WARN, OK, OK, WARN, CRIT] && (frac - 34.0 / 60.0).abs() < 1e-5)
        );
        assert_eq!(s.ends, Some(("200".into(), "260".into())));
        assert_eq!(pill(&s), ("Stable · UPS load 7 %".into(), OK));
        match &s.card {
            Some(Card::Spark {
                series,
                now_idx,
                min,
                max,
                color,
                caption,
                ..
            }) => {
                assert_eq!(series.len(), 24);
                assert_eq!(series[23], 234.0, "the live sample is the current hour");
                assert_eq!(*now_idx, Some(23));
                assert!(*min <= 229.0 && *max >= 237.0, "padded: {min}..{max}");
                assert_eq!((*color, caption.as_str()), (OK, "24 h"));
            }
            _ => panic!("spark"),
        }
        m.apply(
            Event::Mains {
                volts: 212.0,
                ups_load_pct: 7.0,
            },
            1.0,
        );
        let low = mains(&m, 1.0);
        assert_eq!(pill(&low), ("Low voltage".into(), WARN));
        assert!(matches!(&low.card, Some(Card::Spark { color, .. }) if *color == WARN));
        m.apply(
            Event::Mains {
                volts: 247.0,
                ups_load_pct: 7.0,
            },
            2.0,
        );
        assert_eq!(pill(&mains(&m, 2.0)), ("High voltage".into(), WARN));
        assert_eq!(
            mains_pill(240.0, 7.0),
            ("Stable · UPS load 7 %".to_string(), OK)
        );
        assert_eq!(mains_pill(219.9, 7.0).1, WARN);
    }

    #[test]
    fn load_per_core_colours_each_node() {
        let mut m = Model::new(Thresholds::default());
        let nl = |node: &str, l1: f32, l5: f32, l15: f32, cores: u32| NodeLoad {
            node: node.into(),
            load1: l1,
            load5: l5,
            load15: l15,
            cores,
        };
        m.apply(
            Event::Load(vec![
                nl("hp-elitedesk-800-g5-i7", 0.73, 0.6, 0.5, 8),
                nl("raspberrypi-5-8gb-2", 3.8, 3.0, 2.2, 4),
                nl("raspberrypi-5-8gb-3", 2.9, 2.5, 2.0, 4),
            ]),
            0.0,
        );
        let s = load(&m, 0.0);
        assert_eq!(
            (s.title.as_str(), s.hero.as_str(), s.hero_scale),
            ("LOAD", "3.80", 1.0)
        );
        assert!(matches!(&s.ring.kind, RingKind::Discrete(c) if c == &vec![OK, CRIT, WARN]));
        assert_eq!(pill(&s), ("raspberrypi-5-8gb-2 busiest".into(), TEXT2));
        match &s.card {
            Some(Card::Stats(items)) => {
                assert_eq!(items.len(), 3);
                assert_eq!(items[0], ("2.48".to_string(), "1 min".to_string(), TEXT));
                assert_eq!(items[1], ("2.03".to_string(), "5 min".to_string(), TEXT));
                assert_eq!(items[2], ("1.57".to_string(), "15 min".to_string(), TEXT));
            }
            _ => panic!("stats"),
        }
        m.apply(Event::Load(vec![nl("big", 12.5, 1.0, 1.0, 16)]), 1.0);
        let big = load(&m, 1.0);
        assert_eq!((big.hero.as_str(), big.hero_scale), ("12.50", 0.82));
        assert_eq!(load_color(0.69), OK);
        assert_eq!(load_color(0.7), WARN);
        assert_eq!(load_color(0.9), CRIT);
    }
}
