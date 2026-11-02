//! The calibration test pattern: unmistakable up, number, and a corner dot for mirroring.

use rackscreen_core::p2::font::FontId;
use rackscreen_core::p2::text::{middle_baseline, run_centred};
use rackscreen_core::scene::{Drawable, Scene};
use rackscreen_core::theme::layout::{CX, CY};
use rackscreen_core::theme::p2::{AMBER, TEXT};
use rackscreen_core::theme::Role;

pub fn test_pattern(index: usize, _role: Role) -> Scene {
    let mut s = Scene::new();
    s.push(Drawable::Arc {
        cx: CX,
        cy: CY,
        r: 106.0,
        width: 9.0,
        start_deg: 0.0,
        sweep_deg: 360.0,
        color: AMBER,
        alpha: 1.0,
    });
    s.push(Drawable::Icon {
        name: "arrow-up",
        cx: CX,
        cy: CY - 14.0,
        size: 120.0,
        color: TEXT,
        alpha: 1.0,
        scale: 1.0,
        dy: 0.0,
    });
    s.push(run_centred(
        CX,
        middle_baseline(FontId::Bold, 24.0, 182.0),
        &format!("{}", index + 1),
        FontId::Bold,
        24.0,
        0.0,
        TEXT,
        1.0,
    ));
    // top-right marker: visible mirror indicator
    s.push(Drawable::Ellipse {
        cx: 186.0,
        cy: 54.0,
        rx: 7.0,
        ry: 7.0,
        color: AMBER,
        alpha: 1.0,
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
            .any(|d| matches!(d, Drawable::Run { text, .. } if text == "3")));
        assert!(s
            .items
            .iter()
            .any(|d| matches!(d, Drawable::Ellipse { cx, cy, .. } if *cx > 120.0 && *cy < 120.0)));
    }
}
