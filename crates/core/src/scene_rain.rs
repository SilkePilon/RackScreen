//! Rain role: the Buienradar nowcast, two hours in five-minute slots.

use crate::anim::{breathe, Secs};
use crate::model::Model;
use crate::scene::{badge, icon_at, Drawable, Scene, SegState};
use crate::theme::layout::*;
use crate::theme::{Color, BLUE, GREY, VIOLET};

pub const RAIN_SLOTS: usize = 24;
/// Two segments per slot.
pub const RAIN_SEGS: usize = 48;
pub const SLOT_SECS: i64 = 300;
/// A slot at or above this counts as rain.
pub const WET_MM: f32 = 0.1;
/// Rain moving inside this many minutes while it is dry splashes the umbrella.
pub const SOON_MINUTES: u32 = 15;

/// The slots still ahead: elapsed ones are dropped from the front so slot 0
/// is always the current five minutes.
pub fn current_slots(from: i64, mm_per_h: &[f32], unix_now: i64) -> Vec<f32> {
    let elapsed = ((unix_now - from).max(0) / SLOT_SECS) as usize;
    mm_per_h.iter().copied().skip(elapsed).collect()
}

/// Minutes until the first wet slot, `None` when the window is dry.
pub fn first_wet_minutes(slots: &[f32]) -> Option<u32> {
    slots
        .iter()
        .position(|v| *v >= WET_MM)
        .map(|i| (i as i64 * SLOT_SECS / 60) as u32)
}

/// Colour and alpha for an intensity; `None` for dry.
pub fn rain_color(mm: f32) -> Option<(Color, f32)> {
    if mm < WET_MM {
        None
    } else if mm < 1.0 {
        Some((BLUE, 0.5))
    } else if mm < 5.0 {
        Some((BLUE.mix(VIOLET, (mm - 1.0) / 4.0), 1.0))
    } else {
        Some((VIOLET, 1.0))
    }
}

/// Two segments per slot, slot 0 at 12 o'clock breathing.
pub fn rain_states(slots: &[f32], now: Secs) -> Vec<SegState> {
    let mut out = vec![SegState::Off; RAIN_SEGS];
    for (i, mm) in slots.iter().enumerate().take(RAIN_SLOTS) {
        let Some((color, alpha)) = rain_color(*mm) else {
            continue;
        };
        let alpha = if i == 0 {
            alpha * breathe(now, 2.4)
        } else {
            alpha
        };
        out[i * 2] = SegState::On(color, alpha);
        out[i * 2 + 1] = SegState::On(color, alpha);
    }
    out
}

pub fn rain_scene(model: &Model, now: Secs) -> Scene {
    let r = model.rain();
    let slots = current_slots(r.from, &r.mm_per_h, model.unix_now());
    let wet_ahead = first_wet_minutes(&slots);
    let mut s = Scene::new();
    s.push(Drawable::Ring {
        cx: CX,
        cy: CY,
        radius: RING_R,
        n: RAIN_SEGS,
        states: rain_states(&slots, now),
        pitch_deg: 360.0 / RAIN_SEGS as f32,
        start_deg: 0.0,
    });
    let icon_color = if wet_ahead.is_some() { BLUE } else { GREY };
    s.push(icon_at("cloud-rain", ICON_CY, ICON_SIZE, icon_color, 1.0));
    let (text, stroke) = match (slots.first().copied(), wet_ahead) {
        (Some(mm), _) if mm >= WET_MM => (format!("{mm:.1} mm"), BLUE),
        (_, Some(mins)) => (format!("{mins} min"), BLUE),
        _ => ("DRY".to_string(), GREY),
    };
    s.push(badge(BADGE_CY, stroke, text));
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::Event;
    use crate::model::Thresholds;

    fn slots(wet: &[(usize, f32)]) -> Vec<f32> {
        let mut v = vec![0.0f32; 24];
        for (i, mm) in wet {
            v[*i] = *mm;
        }
        v
    }

    #[test]
    fn elapsed_slots_drop_off_the_front() {
        let s = slots(&[(3, 2.0)]);
        assert_eq!(current_slots(1000, &s, 1000).len(), 24);
        let later = current_slots(1000, &s, 1000 + 2 * SLOT_SECS + 10);
        assert_eq!(later.len(), 22);
        assert_eq!(later[1], 2.0);
        assert!(current_slots(1000, &s, 1000 + 30 * SLOT_SECS).is_empty());
        assert_eq!(
            current_slots(1000, &s, 900).len(),
            24,
            "clock behind: nothing dropped"
        );
    }

    #[test]
    fn first_wet_and_colours() {
        assert_eq!(first_wet_minutes(&slots(&[(5, 0.3)])), Some(25));
        assert_eq!(first_wet_minutes(&slots(&[(0, 1.0)])), Some(0));
        assert_eq!(first_wet_minutes(&slots(&[])), None);
        assert_eq!(
            first_wet_minutes(&slots(&[(2, 0.05)])),
            None,
            "below the wet threshold"
        );
        assert_eq!(rain_color(0.0), None);
        assert_eq!(rain_color(0.5), Some((BLUE, 0.5)));
        assert_eq!(rain_color(1.0), Some((BLUE, 1.0)));
        assert_eq!(rain_color(3.0), Some((BLUE.mix(VIOLET, 0.5), 1.0)));
        assert_eq!(rain_color(7.0), Some((VIOLET, 1.0)));
    }

    #[test]
    fn ring_has_two_segments_per_slot_and_the_current_slot_breathes() {
        let v = rain_states(&slots(&[(0, 2.0), (1, 6.0)]), 0.6);
        assert_eq!(v.len(), RAIN_SEGS);
        assert!(matches!(v[0], SegState::On(_, a) if a < 1.0));
        assert!(matches!(v[1], SegState::On(_, a) if a < 1.0));
        assert!(matches!(v[2], SegState::On(c, a) if c == VIOLET && a == 1.0));
        assert!(matches!(v[3], SegState::On(c, _) if c == VIOLET));
        assert_eq!(v[4], SegState::Off);
    }

    fn model(from: i64, now: i64, s: Vec<f32>) -> Model {
        let mut m = Model::new(Thresholds::default());
        m.set_unix_now(now);
        m.apply(Event::Rain { from, mm_per_h: s }, 0.0);
        m
    }

    fn badge_text(s: &Scene) -> (String, Color) {
        s.items
            .iter()
            .find_map(|d| match d {
                Drawable::Badge { text, stroke, .. } => Some((text.clone(), *stroke)),
                _ => None,
            })
            .unwrap()
    }

    #[test]
    fn badge_says_dry_minutes_or_millimetres() {
        assert_eq!(
            badge_text(&rain_scene(&model(0, 0, slots(&[])), 1.0)),
            ("DRY".into(), GREY)
        );
        assert_eq!(
            badge_text(&rain_scene(&model(0, 0, slots(&[(5, 1.0)])), 1.0)),
            ("25 min".into(), BLUE)
        );
        assert_eq!(
            badge_text(&rain_scene(&model(0, 0, slots(&[(0, 2.3)])), 1.0)),
            ("2.3 mm".into(), BLUE)
        );
        // ten minutes later the same nowcast says 15 min
        assert_eq!(
            badge_text(&rain_scene(&model(0, 600, slots(&[(5, 1.0)])), 1.0)).0,
            "15 min"
        );
        let dry = rain_scene(&model(0, 0, slots(&[])), 1.0);
        assert!(dry.items.iter().any(
            |d| matches!(d, Drawable::Icon { name: "cloud-rain", color, .. } if *color == GREY)
        ));
    }
}
