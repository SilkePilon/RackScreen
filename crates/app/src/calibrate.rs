//! The calibration test pattern: unmistakable up, number, and a corner dot for mirroring.

use rackscreen_core::scene::{Drawable, Scene, SegState};
use rackscreen_core::theme::layout::{BADGE_CY, CX, CY, RING_R, SEG_N};
use rackscreen_core::theme::{Role, AMBER, BADGE_FILL, WHITE};

pub fn test_pattern(index: usize, role: Role) -> Scene {
    let mut s = Scene::new();
    s.push(Drawable::Ring {
        cx: CX,
        cy: CY,
        radius: RING_R,
        n: SEG_N,
        states: vec![SegState::On(role.accent(), 1.0); SEG_N],
        pitch_deg: 6.0,
        start_deg: 0.0,
    });
    s.push(Drawable::Icon {
        name: "arrow-up",
        cx: CX,
        cy: CY - 14.0,
        size: 120.0,
        color: WHITE,
        alpha: 1.0,
        scale: 1.0,
        dy: 0.0,
    });
    s.push(Drawable::Badge {
        cx: CX,
        cy: BADGE_CY + 12.0,
        w: 40.0,
        h: 26.0,
        radius: 7.0,
        stroke: role.accent(),
        fill: BADGE_FILL,
        text: format!("{}", index + 1),
        text_px: 15.0,
        text_color: WHITE,
        alpha: 1.0,
    });
    // top-right marker: visible mirror indicator
    s.push(Drawable::Dots {
        cx: 186.0,
        cy: 54.0,
        spacing: 0.0,
        r: 7.0,
        colors: vec![AMBER],
    });
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pattern_has_arrow_number_and_marker() {
        let s = test_pattern(2, Role::Pods);
        assert!(s.items.iter().any(|d| matches!(
            d,
            Drawable::Icon {
                name: "arrow-up",
                ..
            }
        )));
        assert!(s
            .items
            .iter()
            .any(|d| matches!(d, Drawable::Badge { text, .. } if text == "3")));
        assert!(s
            .items
            .iter()
            .any(|d| matches!(d, Drawable::Dots { cx, cy, .. } if *cx > 120.0 && *cy < 120.0)));
        assert_eq!(s.lit_count(), 60);
    }
}
