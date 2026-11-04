//! Four-screen pixel shows: original pixel-art scenes played across all
//! panels as one wide world. Each show is a pure function of the 12 fps
//! frame index `f` and the quantised local time `t`.

mod alpine;
mod aurora;
mod birds;
mod black_hole;
mod caravan;
mod day;
mod deep_sea;
mod desert_night;
mod dino_chase;
mod dragon;
mod fireworks;
mod freight;
mod hyperspace;
mod launch;
mod mech;
mod moon_landing;
mod paper_plane;
mod pirates;
pub mod schedule;
mod scifi;
mod space;
mod storm;
mod ufo;
pub mod world;

mod boulder;
mod fx;
mod heist;
mod kaiju;
mod shark;
mod time_car;

use world::{Layout, World, WorldFrame};

/// Frames per second every show is drawn at ("on twos").
pub const FPS: f64 = 12.0;

#[derive(Debug)]
pub struct ShowDef {
    pub id: &'static str,
    pub name: &'static str,
    /// Length in seconds.
    pub secs: f64,
    pub draw: fn(&mut World, f: u32, t: f64),
}

/// Every show, in spec order.
pub const SHOWS: &[ShowDef] = &[
    ShowDef {
        id: "freight",
        name: "Dusk freight",
        secs: freight::SECS,
        draw: freight::draw,
    },
    ShowDef {
        id: "deep-sea",
        name: "Deep sea",
        secs: deep_sea::SECS,
        draw: deep_sea::draw,
    },
    ShowDef {
        id: "paper-plane",
        name: "Paper plane",
        secs: paper_plane::SECS,
        draw: paper_plane::draw,
    },
    ShowDef {
        id: "storm",
        name: "Thunderstorm",
        secs: storm::SECS,
        draw: storm::draw,
    },
    ShowDef {
        id: "day",
        name: "Day in a minute",
        secs: day::SECS,
        draw: day::draw,
    },
    ShowDef {
        id: "space",
        name: "Space cruise",
        secs: space::SECS,
        draw: space::draw,
    },
    ShowDef {
        id: "birds",
        name: "Birds at sunset",
        secs: birds::SECS,
        draw: birds::draw,
    },
    ShowDef {
        id: "fireworks",
        name: "Fireworks",
        secs: fireworks::SECS,
        draw: fireworks::draw,
    },
    ShowDef {
        id: "alpine",
        name: "Mountains: alpine dawn",
        secs: alpine::SECS,
        draw: alpine::draw,
    },
    ShowDef {
        id: "caravan",
        name: "Desert: caravan",
        secs: caravan::SECS,
        draw: caravan::draw,
    },
    ShowDef {
        id: "desert-night",
        name: "Desert: night tumbleweed",
        secs: desert_night::SECS,
        draw: desert_night::draw,
    },
    ShowDef {
        id: "boulder",
        name: "Temple run",
        secs: boulder::SECS,
        draw: boulder::draw,
    },
    ShowDef {
        id: "time-car",
        name: "Time machine",
        secs: time_car::SECS,
        draw: time_car::draw,
    },
    ShowDef {
        id: "kaiju",
        name: "Monster in the harbour",
        secs: kaiju::SECS,
        draw: kaiju::draw,
    },
    ShowDef {
        id: "heist",
        name: "The heist",
        secs: heist::SECS,
        draw: heist::draw,
    },
    ShowDef {
        id: "shark",
        name: "Beach day",
        secs: shark::SECS,
        draw: shark::draw,
    },
    ShowDef {
        id: "dino-chase",
        name: "Dinosaur chase",
        secs: dino_chase::SECS,
        draw: dino_chase::draw,
    },
    ShowDef {
        id: "dragon",
        name: "Dragon over the castle",
        secs: dragon::SECS,
        draw: dragon::draw,
    },
    ShowDef {
        id: "pirates",
        name: "Broadside",
        secs: pirates::SECS,
        draw: pirates::draw,
    },
    ShowDef {
        id: "aurora",
        name: "Northern lights",
        secs: aurora::SECS,
        draw: aurora::draw,
    },
    ShowDef {
        id: "launch",
        name: "Lift-off",
        secs: launch::SECS,
        draw: launch::draw,
    },
    ShowDef {
        id: "hyperspace",
        name: "Jump to hyperspace",
        secs: hyperspace::SECS,
        draw: hyperspace::draw,
    },
    ShowDef {
        id: "ufo",
        name: "Close encounter",
        secs: ufo::SECS,
        draw: ufo::draw,
    },
    ShowDef {
        id: "moon-landing",
        name: "Lunar landing",
        secs: moon_landing::SECS,
        draw: moon_landing::draw,
    },
    ShowDef {
        id: "mech",
        name: "Giant robot at dusk",
        secs: mech::SECS,
        draw: mech::draw,
    },
    ShowDef {
        id: "black-hole",
        name: "Event horizon",
        secs: black_hole::SECS,
        draw: black_hole::draw,
    },
];

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
        assert_eq!((d.name, d.secs), ("Dusk freight", 12.0));
        assert!(by_id("nope").is_none());
    }

    #[test]
    fn every_show_is_registered_in_spec_order() {
        let got: Vec<_> = SHOWS[1..].iter().map(|d| (d.id, d.name, d.secs)).collect();
        assert_eq!(
            got,
            [
                ("deep-sea", "Deep sea", 10.0),
                ("paper-plane", "Paper plane", 8.0),
                ("storm", "Thunderstorm", 8.0),
                ("day", "Day in a minute", 12.0),
                ("space", "Space cruise", 8.0),
                ("birds", "Birds at sunset", 9.0),
                ("fireworks", "Fireworks", 8.0),
                ("alpine", "Mountains: alpine dawn", 10.0),
                ("caravan", "Desert: caravan", 10.0),
                ("desert-night", "Desert: night tumbleweed", 9.0),
                ("boulder", "Temple run", 10.0),
                ("time-car", "Time machine", 10.0),
                ("kaiju", "Monster in the harbour", 11.0),
                ("heist", "The heist", 10.0),
                ("shark", "Beach day", 10.0),
                ("dino-chase", "Dinosaur chase", 10.0),
                ("dragon", "Dragon over the castle", 10.0),
                ("pirates", "Broadside", 11.0),
                ("aurora", "Northern lights", 10.0),
                ("launch", "Lift-off", 12.0),
                ("hyperspace", "Jump to hyperspace", 10.0),
                ("ufo", "Close encounter", 12.0),
                ("moon-landing", "Lunar landing", 12.0),
                ("mech", "Giant robot at dusk", 11.0),
                ("black-hole", "Event horizon", 10.0),
            ]
        );
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
