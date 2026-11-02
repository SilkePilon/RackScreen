//! Rack hardware and the plain fill-ring roles: thermal, net, ups, wind,
//! rain, aqi, sun, moon, renewable.

use crate::anim::Secs;
use crate::model::Model;
use crate::p2::chart::Card;
use crate::p2::ring::RingSpec;
use crate::p2::roles::short_node;
use crate::p2::screen::P2Screen;
use crate::theme::p2::{CRIT, OK, TEXT2, WARN};
use crate::theme::Color;

/// `OK` under `hot - 5`, `WARN` from there, `CRIT` from `hot + 5` (65 / 75 with
/// the default `hot_temp` of 70).
pub fn temp_color(c: f32, hot: f32) -> Color {
    if c >= hot + 5.0 {
        CRIT
    } else if c >= hot - 5.0 {
        WARN
    } else {
        OK
    }
}

pub fn thermal(model: &Model, _now: Secs) -> P2Screen {
    let st = model.state();
    let hot = model.thresholds().hot_temp;
    let hottest = st
        .temps
        .iter()
        .enumerate()
        .max_by(|a, b| a.1 .1.total_cmp(&b.1 .1));
    let (idx, name, max) = match hottest {
        Some((i, (n, c))) => (Some(i), n.as_str(), *c),
        None => (None, "", 0.0),
    };
    let mut s = P2Screen::new("THERMAL", format!("{max:.0}"));
    s.unit = Some("°".into());
    s.ring = RingSpec::fill(((max - 30.0) / 55.0).clamp(0.0, 1.0), temp_color(max, hot));
    s.pill = Some((format!("{name} hottest"), TEXT2));
    s.card = Some(Card::NodeBars {
        values: st.temps.iter().map(|(_, c)| *c).collect(),
        min: 30.0,
        max: 85.0,
        labels: st.temps.iter().map(|(n, _)| short_node(n)).collect(),
        colors: st.temps.iter().map(|(_, c)| temp_color(*c, hot)).collect(),
        highlight: idx,
    });
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::Event;
    use crate::model::Thresholds;
    use crate::p2::ring::RingKind;
    use crate::theme::p2::{CRIT, OK, TEXT2, WARN};

    #[test]
    fn thermal_names_the_hottest_node_and_bars_the_rest() {
        let mut m = Model::new(Thresholds::default());
        m.apply(
            Event::NodeTemps(vec![
                ("hp-elitedesk-800-g5-i7".into(), 46.0),
                ("raspberrypi-5-8gb-1".into(), 54.0),
                ("raspberrypi-5-8gb-2".into(), 68.0),
            ]),
            0.0,
        );
        let s = thermal(&m, 0.0);
        assert_eq!(
            (s.title.as_str(), s.hero.as_str(), s.unit.as_deref()),
            ("THERMAL", "68", Some("°"))
        );
        assert!(
            matches!(s.ring.kind, RingKind::Fill { frac, color } if (frac - 38.0 / 55.0).abs() < 1e-5 && color == WARN)
        );
        assert_eq!(s.pill, Some(("raspberrypi-5-8gb-2 hottest".into(), TEXT2)));
        match &s.card {
            Some(Card::NodeBars {
                values,
                min,
                max,
                labels,
                colors,
                highlight,
            }) => {
                assert_eq!(values, &vec![46.0, 54.0, 68.0]);
                assert_eq!((*min, *max, *highlight), (30.0, 85.0, Some(2)));
                assert_eq!(labels, &vec!["i7".to_string(), "r1".into(), "r2".into()]);
                assert_eq!(colors, &vec![OK, OK, WARN]);
            }
            _ => panic!("node bars"),
        }
        assert_eq!(temp_color(76.0, 70.0), CRIT);
        assert_eq!(temp_color(64.9, 70.0), OK);
    }
}
