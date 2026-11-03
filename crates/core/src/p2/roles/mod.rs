//! Role builders: `Model` → `P2Screen`, plain data the composer draws.

pub mod cluster;
pub mod github;
pub mod media;
pub mod power_mix;
pub mod rack;
pub mod sky;
pub mod system;

use crate::anim::Secs;
use crate::model::Model;
use crate::p2::screen::P2Screen;
use crate::theme::Role;

/// The P2 screen for a role with live data. `None` for roles that still draw
/// their legacy scene, and for power-mix, which draws itself.
pub fn screen(model: &Model, role: Role, now: Secs) -> Option<P2Screen> {
    let mut screen = match role {
        Role::Cpu => cluster::cpu(model, now),
        Role::Mem => cluster::mem(model, now),
        Role::Storage => cluster::storage(model, now),
        Role::Pods => cluster::pods(model, now),
        Role::Health => cluster::health(model, now),
        Role::Deploys => cluster::deploys(model, now),
        Role::Weather => sky::weather(model, now),
        Role::Iss => sky::iss(model, now),
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
        Role::Traffic => rack::traffic(model, now),
        Role::Restarts => rack::restarts(model, now),
        Role::Uptime => rack::uptime(model, now),
        Role::Fans => rack::fans(model, now),
        Role::Mains => rack::mains(model, now),
        Role::Load => rack::load(model, now),
        Role::Media => media::media(model, now),
        Role::Ci => github::ci(model, now),
        Role::Jellyfin => media::jellyfin(model, now),
        Role::PowerMix | Role::Gif(_) => return None,
    };
    screen.icon = role.icon();
    Some(screen)
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

    use crate::p2::layout::{HERO_BASELINE, HERO_BASELINE_UNIT_BELOW};
    use crate::p2::text::{hero_fit_scale, hero_fits};

    #[test]
    fn worst_case_heroes_fit_the_ring() {
        // (hero, superscript unit, the role's hero_scale, unit below?)
        for (text, unit, scale, below) in [
            ("100", Some("%"), 1.0, false), // cpu, mem, storage, ups, renewable, moon
            ("999", None, 1.0, false),      // pods, deploys, contributions
            ("16/16", None, 0.82, false),   // cluster
            ("-12", Some("°"), 1.0, false), // weather below zero
            ("59:59", None, 0.82, false),   // iss
            ("1000", None, 1.0, true),      // net Mb/s
            ("120", None, 1.0, true),       // wind, aqi, rain minutes
            ("23:59", None, 0.82, false),   // sun
            ("9999", None, 1.0, false),     // media library, restarts
            ("99.9", None, 1.0, true),      // traffic req/s
            ("100", Some("%"), 1.0, false), // fans
            ("260", Some("V"), 1.0, false), // mains
            ("82", Some("d"), 1.0, false),  // uptime
            ("10.52", None, 0.82, false),   // load from 10 up
            ("12/12", None, 0.82, false),   // ci
            ("1:41:54", None, 0.62, true),  // jellyfin h:mm:ss
            ("59:59", None, 0.82, true),    // jellyfin m:ss
        ] {
            let base = if below {
                HERO_BASELINE_UNIT_BELOW
            } else {
                HERO_BASELINE
            };
            let k = hero_fit_scale(text, unit, scale, base);
            assert!(k <= scale && k > 0.4, "{text}: {k}");
            assert!(hero_fits(text, unit, k, base), "{text} {unit:?} at {k}");
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
