//! Role builders: `Model` → `P2Screen`, plain data the composer draws.

pub mod cluster;
pub mod energy;
pub mod github;
pub mod power_mix;
pub mod sky;
pub mod system;

use crate::anim::Secs;
use crate::model::Model;
use crate::p2::screen::P2Screen;
use crate::theme::Role;

/// The P2 screen for a role with live data. `None` for roles that still draw
/// their legacy scene, and for power-mix and face, which draw themselves.
pub fn screen(model: &Model, role: Role, now: Secs) -> Option<P2Screen> {
    Some(match role {
        Role::Cpu => cluster::cpu(model, now),
        Role::Mem => cluster::mem(model, now),
        Role::Storage => cluster::storage(model, now),
        Role::Pods => cluster::pods(model, now),
        Role::Health => cluster::health(model, now),
        Role::Deploys => cluster::deploys(model, now),
        Role::Weather => sky::weather(model, now),
        Role::Iss => sky::iss(model, now),
        Role::Price => energy::price(model, now),
        Role::Carbon => energy::carbon(model, now),
        Role::GhActivity => github::github(model, now),
        Role::Thermal => system::thermal(model, now),
        Role::Net => system::net(model, now),
        Role::Ups => system::ups(model, now),
        Role::Wind => system::wind(model, now),
        Role::Rain => system::rain(model, now),
        Role::Aqi => system::aqi(model, now),
        Role::Sun => system::sun(model, now),
        Role::Moon => system::moon(model, now),
        Role::Renewable => system::renewable(model, now),
        Role::PowerMix | Role::Face => return None,
    })
}

/// A two-character node label for the node bars: the last `-` token, prefixed
/// with the name's first letter when the token is a bare number.
pub fn short_node(name: &str) -> String {
    let last = name.rsplit('-').next().unwrap_or(name);
    if !last.is_empty() && last.chars().all(|c| c.is_ascii_digit()) {
        let first = name.chars().next().unwrap_or('n');
        format!("{first}{last}")
    } else {
        last.chars().take(2).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::p2::layout::HERO_MAX_W;
    use crate::p2::text::hero_width;

    #[test]
    fn worst_case_heroes_fit_180_px() {
        for (text, scale) in [
            ("100", 1.0),     // cpu, mem, storage, ups, renewable, moon
            ("999", 1.0),     // pods, deploys, contributions, carbon
            ("16/16", 0.82),  // cluster
            ("-12", 1.0),     // weather below zero
            ("59:59", 0.82),  // iss
            ("-0.123", 0.78), // price
            ("1000", 1.0),    // net Mb/s
            ("120", 1.0),     // wind, aqi, rain minutes
            ("23:59", 0.82),  // sun
        ] {
            let w = hero_width(text, scale);
            assert!(w <= HERO_MAX_W, "{text} at {scale}: {w} px");
        }
    }

    #[test]
    fn node_labels_are_two_characters() {
        assert_eq!(short_node("raspberrypi-5-8gb-3"), "r3");
        assert_eq!(short_node("hp-elitedesk-800-g5-i7"), "i7");
        assert_eq!(short_node("node"), "no");
        assert_eq!(short_node("k3s-worker-12"), "k12");
    }
}
