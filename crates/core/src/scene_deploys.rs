//! Deploys role: one arc per Argo CD application, coloured by sync and health.

use crate::anim::{breathe, Secs};
use crate::event::{App, AppHealth, AppSync};
use crate::model::Model;
use crate::scene::{badge, icon_at, ring, Drawable, Scene, SegState};
use crate::theme::layout::*;
use crate::theme::{Color, AMBER, GREEN, GREY, RED, WHITE};

/// Synced, healthy and idle: the calm state.
pub fn app_ok(a: &App) -> bool {
    a.health == AppHealth::Healthy && a.sync == AppSync::Synced && !a.operating
}

/// Arc colour and whether it breathes.
fn app_color(a: &App) -> (Color, bool) {
    if matches!(a.health, AppHealth::Degraded | AppHealth::Missing) {
        (RED, false)
    } else if matches!(a.health, AppHealth::Suspended | AppHealth::Unknown)
        || a.sync == AppSync::Unknown
    {
        (GREY, false)
    } else if a.health == AppHealth::Progressing || a.sync == AppSync::OutOfSync || a.operating {
        (AMBER, true)
    } else {
        (GREEN, false)
    }
}

/// Equal sections in name order, an unlit gap between them when there is room.
pub fn app_states(apps: &[App], now: Secs) -> Vec<SegState> {
    let n = SEG_N;
    if apps.is_empty() {
        return vec![SegState::Off; n];
    }
    let count = apps.len();
    let per = (n / count).max(1);
    let gap = if per >= 3 { 1 } else { 0 };
    let mut out = vec![SegState::Off; n];
    for (k, app) in apps.iter().enumerate().take(n) {
        let start = k * per;
        let end = if k == count - 1 {
            n
        } else {
            (start + per).min(n)
        };
        let lit_end = end.saturating_sub(gap);
        let (color, breathing) = app_color(app);
        let a = if breathing { breathe(now, 1.2) } else { 1.0 };
        for st in out.iter_mut().take(lit_end).skip(start) {
            *st = SegState::On(color, a);
        }
    }
    out
}

pub fn deploys_scene(model: &Model, now: Secs) -> Scene {
    let apps = &model.apps().apps;
    let ok = apps.iter().filter(|a| app_ok(a)).count();
    let degraded = apps
        .iter()
        .any(|a| matches!(a.health, AppHealth::Degraded | AppHealth::Missing));
    let mut s = Scene::new();
    s.push(ring(RING_R, app_states(apps, now)));
    s.push(icon_at("rocket", ICON_CY, ICON_SIZE, WHITE, 1.0));
    // `0 == 0` is not healthy: an Argo CD reporting no applications at all is
    // an absence, not a green wall.
    let stroke = if apps.is_empty() {
        GREY
    } else if ok == apps.len() {
        GREEN
    } else {
        AMBER
    };
    s.push(badge(BADGE_CY, stroke, format!("{ok}/{}", apps.len())));
    if degraded {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::Event;
    use crate::model::Thresholds;

    fn app(name: &str, sync: AppSync, health: AppHealth, operating: bool) -> App {
        App {
            name: name.into(),
            sync,
            health,
            operating,
        }
    }

    fn sixteen(bad: Option<usize>) -> Vec<App> {
        (0..16)
            .map(|i| {
                if Some(i) == bad {
                    app(
                        &format!("app-{i:02}"),
                        AppSync::OutOfSync,
                        AppHealth::Healthy,
                        false,
                    )
                } else {
                    app(
                        &format!("app-{i:02}"),
                        AppSync::Synced,
                        AppHealth::Healthy,
                        false,
                    )
                }
            })
            .collect()
    }

    #[test]
    fn sixteen_apps_are_sixteen_arcs_with_gaps() {
        let v = app_states(&sixteen(Some(5)), 0.0);
        // 60 / 16 = 3 per app, one of them a gap
        assert!(matches!(v[0], SegState::On(c, _) if c == GREEN));
        assert!(matches!(v[1], SegState::On(c, _) if c == GREEN));
        assert_eq!(v[2], SegState::Off);
        assert!(matches!(v[15], SegState::On(c, a) if c == AMBER && a < 1.0));
        assert_eq!(v[17], SegState::Off);
        // the last app takes the leftover segments up to the end, less its gap
        assert!(matches!(v[58], SegState::On(c, _) if c == GREEN));
        assert_eq!(v[59], SegState::Off, "the gap before the ring wraps");
        assert_eq!(app_states(&[], 0.0), vec![SegState::Off; 60]);
    }

    #[test]
    fn colours_follow_health_then_sync_then_operation() {
        let red = app("a", AppSync::Synced, AppHealth::Degraded, false);
        let grey = app("a", AppSync::Synced, AppHealth::Suspended, false);
        let amber = app("a", AppSync::Synced, AppHealth::Healthy, true);
        let green = app("a", AppSync::Synced, AppHealth::Healthy, false);
        assert_eq!(app_color(&red), (RED, false));
        assert_eq!(app_color(&grey), (GREY, false));
        assert_eq!(app_color(&amber), (AMBER, true));
        assert_eq!(app_color(&green), (GREEN, false));
        assert!(app_ok(&green) && !app_ok(&amber));
    }

    #[test]
    fn badge_counts_ok_apps_and_a_degraded_one_shows_the_marker() {
        let mut m = Model::new(Thresholds::default());
        m.apply(Event::Apps(sixteen(Some(3))), 0.0);
        let s = deploys_scene(&m, 0.5);
        let (text, stroke) = s
            .items
            .iter()
            .find_map(|d| match d {
                Drawable::Badge { text, stroke, .. } => Some((text.clone(), *stroke)),
                _ => None,
            })
            .unwrap();
        assert_eq!(text, "15/16");
        assert_eq!(stroke, AMBER);
        assert!(!s.items.iter().any(|d| matches!(d, Drawable::Dots { .. })));
        let mut apps = sixteen(None);
        apps[0].health = AppHealth::Degraded;
        m.apply(Event::Apps(apps), 1.0);
        let s = deploys_scene(&m, 1.5);
        assert!(s.items.iter().any(|d| matches!(d, Drawable::Dots { .. })));
        // an Argo CD with nothing to report is grey, not a green 0/0
        m.apply(Event::Apps(Vec::new()), 2.0);
        let s = deploys_scene(&m, 2.5);
        let (text, stroke) = s
            .items
            .iter()
            .find_map(|d| match d {
                Drawable::Badge { text, stroke, .. } => Some((text.clone(), *stroke)),
                _ => None,
            })
            .unwrap();
        assert_eq!(text, "0/0");
        assert_eq!(stroke, GREY);
    }
}
