//! `/api/roles` and `/api/shows`: what a screen can show.

use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use rackscreen_app::config::Config;
use rackscreen_core::theme::Role;
use serde::Serialize;

use crate::{ApiError, ApiResult, AppState};

#[derive(Serialize)]
pub struct RoleInfo {
    pub name: String,
    pub title: String,
    pub group: &'static str,
    pub description: String,
    pub needs: Option<&'static str>,
}

pub fn group(role: Role) -> &'static str {
    match role {
        Role::PowerMix | Role::Renewable => "Energy",
        r if r.is_sky() => "Sky",
        Role::GhActivity | Role::Ci => "GitHub",
        Role::Media | Role::Jellyfin | Role::Music => "Media",
        Role::Gif(_) => "GIFs",
        Role::Printer | Role::Ha(_) => "Home",
        _ => "Cluster",
    }
}

pub fn description(role: Role) -> &'static str {
    match role {
        Role::Cpu => {
            "Cluster CPU load with the last minute as a sparkline; amber over the threshold."
        }
        Role::Mem => "Cluster memory use, GB used of total, the last 90 s as a sparkline.",
        Role::Pods => "Running pods; the ring split into running, pending and failed.",
        Role::Health => {
            "A segment per node, alerts and not-ready counts; doubles as a download monitor."
        }
        Role::Thermal => "The hottest node, amber and red near the threshold; a bar per node.",
        Role::Storage => "Used Longhorn capacity split per volume; a degraded volume turns red.",
        Role::PowerMix => "The grid's power mix by source from Electricity Maps.",
        Role::Renewable => {
            "The renewable share, the fossil-free share in the pill, today as a sparkline."
        }
        Role::GhActivity => {
            "Today's GitHub contributions against your best day, this week as bars."
        }
        Role::Weather => {
            "Temperature on the day's range, the condition and rain ahead, the next 12 h."
        }
        Role::Wind => {
            "Wind speed, gusts in the pill, a compass strip that slides to the direction."
        }
        Role::Aqi => "The European air quality index on its six bands, the last 24 h.",
        Role::Rain => "Minutes until rain (Buienradar), the next two hours as five-minute bars.",
        Role::Sun => {
            "The next sunrise or sunset and its countdown, how far the day or night has run."
        }
        Role::Moon => "The illuminated fraction, the phase, days to full and new moon.",
        Role::Iss => "A countdown to the next ISS pass with its start, duration and height.",
        Role::Ups => "Battery charge, on mains or on battery, runtime and load (nut-exporter).",
        Role::Net => "Total traffic, download and upload in the pill, download as a sparkline.",
        Role::Deploys => "Argo CD syncs today, one segment per application, syncs per hour.",
        Role::Media => {
            "Movies, series and artists on the ring; the active arr download in the card."
        }
        Role::Traffic => {
            "Traefik requests per second, the three busiest services, the error share."
        }
        Role::Restarts => {
            "Container restarts in 24 h, one segment per hour, the worst pod in the pill."
        }
        Role::Uptime => "The longest node uptime, a segment per node, amber for a recent reboot.",
        Role::Fans => "Fan power in % of the busiest fan, a bar per node with a fan.",
        Role::Mains => "UPS input voltage on six bands over 200–260 V, the last 24 h.",
        Role::Load => "The highest 1-minute load, a segment per node by load per core.",
        Role::Ci => "The last twelve GitHub Actions runs as segments, passes over total.",
        Role::Jellyfin => "Time left on what is playing on Jellyfin, title and viewer in the card.",
        Role::Music => "The album cover and track progress from Spotify or Jellyfin.",
        Role::Gif(_) => "A GIF over the whole panel, from the gifs list.",
        Role::Printer => {
            "A Bambu Lab print: progress on the ring in the filament colour, time left, layer and temperatures."
        }
        Role::Ha(_) => {
            "A Home Assistant entity: a number with its sparkline, or a state such as on, open or home."
        }
    }
}

/// The missing setting that keeps `role` from showing data, if any.
pub fn needs(cfg: &Config, role: Role) -> Option<&'static str> {
    match role {
        Role::PowerMix | Role::Renewable => (!cfg.electricity.enabled
            || cfg.electricity.token.trim().is_empty())
        .then_some("Electricity Maps token"),
        r if r.is_sky() => {
            if cfg.location().is_none() {
                Some("Location")
            } else {
                match r {
                    Role::Weather | Role::Wind | Role::Aqi => {
                        (!cfg.weather.enabled).then_some("Weather on")
                    }
                    Role::Rain => (!cfg.rain.enabled).then_some("Rain on"),
                    Role::Iss => (!cfg.iss.enabled).then_some("ISS on"),
                    _ => None,
                }
            }
        }
        Role::GhActivity | Role::Ci => {
            (!cfg.github.enabled || cfg.github.token.trim().is_empty()).then_some("GitHub token")
        }
        Role::Deploys => (!cfg.argocd.enabled).then_some("Argo CD on"),
        Role::Jellyfin => {
            (!cfg.jellyfin.enabled || cfg.jellyfin.url.trim().is_empty()).then_some("Jellyfin URL")
        }
        Role::Printer => (!cfg.bambu.ready()).then_some("Bambu printer"),
        Role::Ha(_) => (!cfg.homeassistant.ready()).then_some("Home Assistant token"),
        Role::Music => {
            let jellyfin = cfg.jellyfin.enabled && !cfg.jellyfin.url.trim().is_empty();
            let spotify = cfg.spotify.enabled && !cfg.spotify.client_id.trim().is_empty();
            (!jellyfin && !spotify).then_some("Spotify or Jellyfin")
        }
        _ => None,
    }
}

pub fn role_list(cfg: &Config) -> Vec<RoleInfo> {
    cfg.pickable_roles()
        .into_iter()
        .map(|r| RoleInfo {
            name: cfg.role_name(r),
            title: match r {
                Role::Gif(i) => cfg
                    .gifs
                    .get(i as usize)
                    .map_or("GIF".into(), |g| g.name.clone()),
                Role::Ha(i) => {
                    cfg.homeassistant
                        .entities
                        .get(i as usize)
                        .map_or("Home".into(), |e| {
                            if e.label.trim().is_empty() {
                                e.name.clone()
                            } else {
                                e.label.trim().to_string()
                            }
                        })
                }
                r => title_case(r.title()),
            },
            group: group(r),
            description: description(r).to_string(),
            needs: needs(cfg, r),
        })
        .collect()
}

/// `POWER MIX` to `Power mix`; abbreviations stay upper case.
fn title_case(s: &str) -> String {
    const KEEP: [&str; 5] = ["CPU", "ISS", "UPS", "CI", "GIF"];
    s.split(' ')
        .enumerate()
        .map(|(i, w)| {
            if KEEP.contains(&w) || w == "·" {
                w.to_string()
            } else if i == 0 {
                let mut c = w.chars();
                match c.next() {
                    Some(f) => f.to_string() + &c.as_str().to_lowercase(),
                    None => String::new(),
                }
            } else {
                w.to_lowercase()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub async fn roles(State(state): State<Arc<AppState>>) -> ApiResult<Vec<RoleInfo>> {
    let cfg = Config::load_or_default(&state.hub.config_path)
        .map_err(|e| ApiError::internal(format!("{e:#}")))?;
    Ok(Json(role_list(&cfg)))
}

#[derive(Serialize)]
pub struct ShowInfo {
    pub id: &'static str,
    pub title: &'static str,
}

pub async fn shows() -> ApiResult<Vec<ShowInfo>> {
    Ok(Json(
        rackscreen_core::shows::SHOWS
            .iter()
            .map(|s| ShowInfo {
                id: s.id,
                title: s.name,
            })
            .collect(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_role_is_listed_with_a_group_and_the_gifs_follow() {
        let mut cfg = Config::default();
        cfg.gifs.push(rackscreen_app::config::GifCfg {
            name: "party".into(),
            url: "https://giphy.com/gifs/x-AbCdEf123".into(),
        });
        let list = role_list(&cfg);
        assert_eq!(list.len(), Role::ALL.len() + 1);
        assert_eq!(list[0].name, "cpu");
        assert_eq!(list[0].title, "CPU");
        assert_eq!(list[0].group, "Cluster");
        assert_eq!(list[0].needs, None);
        let last = list.last().unwrap();
        assert_eq!(last.name, "gif:party");
        assert_eq!(last.title, "party");
        assert_eq!(last.group, "GIFs");
        let by = |n: &str| list.iter().find(|r| r.name == n).unwrap();
        assert_eq!(by("power-mix").needs, Some("Electricity Maps token"));
        assert_eq!(by("power-mix").title, "Power mix");
        assert_eq!(by("weather").needs, Some("Location"));
        assert_eq!(by("gh-activity").needs, Some("GitHub token"));
        assert_eq!(by("music").needs, Some("Spotify or Jellyfin"));
        assert_eq!(by("deploys").needs, None, "argocd is on by default");
        assert_eq!(by("restarts").title, "Restarts · 24h");
        assert!(list.iter().all(|r| !r.description.is_empty()));
        cfg.location.lat = Some(1.0);
        cfg.location.lon = Some(2.0);
        assert_eq!(needs(&cfg, Role::Weather), Some("Weather on"));
        assert_eq!(needs(&cfg, Role::Sun), None);
        cfg.weather.enabled = true;
        assert_eq!(needs(&cfg, Role::Weather), None);
    }

    /// The Screens page shows `web/public/roles/<name>.webp` for every
    /// built-in role; `scripts/role-previews.sh` makes them from the goldens.
    #[test]
    fn every_role_has_a_web_preview() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web/public/roles");
        let missing: Vec<_> = Role::ALL
            .iter()
            .map(|r| r.name())
            .filter(|n| !dir.join(format!("{n}.webp")).is_file())
            .collect();
        assert!(
            missing.is_empty(),
            "no preview for {missing:?}: run scripts/role-previews.sh"
        );
    }
}
