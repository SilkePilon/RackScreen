//! Four-screen pixel shows: original pixel-art scenes played across all
//! panels as one wide world. Each show is a pure function of the 12 fps
//! frame index `f` and the quantised local time `t`.

mod freight;
pub mod world;

use world::{Layout, World, WorldFrame};

/// Frames per second every show is drawn at ("on twos").
pub const FPS: f64 = 12.0;

pub struct ShowDef {
    pub id: &'static str,
    pub name: &'static str,
    /// Length in seconds.
    pub secs: f64,
    pub draw: fn(&mut World, f: u32, t: f64),
}

/// Every show, in spec order.
pub const SHOWS: &[ShowDef] = &[ShowDef {
    id: "freight",
    name: "Dusk freight",
    secs: freight::SECS,
    draw: freight::draw,
}];

pub fn by_id(id: &str) -> Option<&'static ShowDef> {
    SHOWS.iter().find(|s| s.id == id)
}

/// Draw `def` at local time `t` (quantised to 12 fps) into a fresh world.
pub fn render(def: &ShowDef, layout: Layout, t: f64) -> WorldFrame {
    let f = (t.max(0.0) * FPS).floor();
    let mut frame = WorldFrame::new(layout.width());
    (def.draw)(
        &mut World {
            frame: &mut frame,
            layout,
        },
        f as u32,
        f / FPS,
    );
    frame
}

#[cfg(test)]
mod tests {
    use super::*;

    const LAYOUTS: [Layout; 2] = [Layout { panels: 4, gap: 46 }, Layout { panels: 3, gap: 46 }];

    fn frames(def: &ShowDef) -> impl Iterator<Item = f64> {
        (0..(def.secs * FPS) as u32).map(|f| f as f64 / FPS)
    }

    #[test]
    fn freight_is_registered() {
        let d = by_id("freight").expect("freight");
        assert_eq!((d.name, d.secs), ("Dusk freight", 9.0));
        assert!(by_id("nope").is_none());
    }

    #[test]
    fn ids_are_unique() {
        for (i, a) in SHOWS.iter().enumerate() {
            assert!(SHOWS[i + 1..].iter().all(|b| b.id != a.id), "{}", a.id);
        }
    }

    #[test]
    fn every_frame_renders_and_paints_the_whole_world() {
        for def in SHOWS {
            for l in LAYOUTS {
                for t in frames(def) {
                    let fr = render(def, l, t);
                    assert_eq!(fr.rgba.len(), (l.width() * 60 * 4) as usize);
                    assert!(
                        fr.rgba.chunks(4).all(|p| p != [0, 0, 0, 0]),
                        "{} left a pixel unpainted at t={t} ({l:?})",
                        def.id
                    );
                }
            }
        }
    }

    #[test]
    fn shows_are_deterministic_and_move() {
        for def in SHOWS {
            for l in LAYOUTS {
                assert_eq!(render(def, l, 1.0), render(def, l, 1.0), "{}", def.id);
                assert_ne!(render(def, l, 0.0), render(def, l, 0.5), "{} moves", def.id);
            }
        }
    }

    #[test]
    fn time_is_quantised_to_twelve_fps() {
        for def in SHOWS {
            let l = LAYOUTS[0];
            assert_eq!(
                render(def, l, 1.0),
                render(def, l, 1.0 + 0.5 / FPS),
                "{}",
                def.id
            );
        }
    }
}
