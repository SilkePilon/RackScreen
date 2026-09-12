//! Pixel bitmaps for the face: source icons (7 wide), eye sprites (7 wide) and
//! the odd dot. `#` is lit, anything else dark.

use crate::theme::Color;

#[derive(Debug, PartialEq)]
pub struct Sprite {
    pub rows: &'static [&'static str],
}

impl Sprite {
    pub fn w(&self) -> usize {
        self.rows.first().map_or(0, |r| r.len())
    }
    pub fn h(&self) -> usize {
        self.rows.len()
    }
    /// Cell `(i, j)` lit? Out of range is dark.
    pub fn on(&self, i: usize, j: usize) -> bool {
        self.rows
            .get(j)
            .and_then(|r| r.as_bytes().get(i))
            .is_some_and(|b| *b == b'#')
    }
}

macro_rules! sprite {
    ($name:ident, $($row:literal),+ $(,)?) => {
        pub static $name: Sprite = Sprite { rows: &[$($row),+] };
    };
}

// ---- source icons, 7 wide ----
sprite!(BOX, "..###..", ".#...#.", "#..#..#", "#.###.#", "#..#..#", ".#.#.#.", "..###..");
sprite!(SERVER, "#######", "#.#...#", "#######", "#.#...#", "#######");
sprite!(BRANCH, ".##....", ".##..##", "..#..##", "..#.#..", "..##...", "..#....", ".##....");
sprite!(ROCKET, "...#...", "..###..", "..###..", "..###..", ".#####.", "#..#..#", "...#...");
sprite!(DOWNLOAD, "...#...", "...#...", ".#####.", "..###..", "...#...", "#.....#", "#######");
sprite!(BOLT, "...##..", "..##...", ".###...", "..####.", "...##..", "..##...", ".##....");
sprite!(BATLOW, "######.", "#.....#", "##....#", "##....#", "#.....#", "######.");
sprite!(BELL, "...#...", "..###..", ".#####.", ".#####.", ".#####.", "#######", "...#...");
sprite!(BELL2, "....#..", "...###.", "..####.", ".#####.", ".#####.", "######.", "...#...");
sprite!(CHECK, "......#", ".....##", "....##.", "#..##..", "##.#...", ".###...", "..#....");
sprite!(CROSS, "#...#", ".#.#.", "..#..", ".#.#.", "#...#");
sprite!(TRI, "...#...", "..#.#..", "..#.#..", ".#...#.", ".#.#.#.", "#.....#", "#######");
sprite!(CPU, ".#.#.#.", "#######", "#.....#", "#..#..#", "#.....#", "#######", ".#.#.#.");
sprite!(MEM, "#######", "#.#.#.#", "#.#.#.#", "#######", ".#.#.#.");
sprite!(DISK, ".#####.", "#.....#", ".#####.", "#.....#", ".#####.", "#.....#", ".#####.");
sprite!(FLAME, "...#...", "..##...", "..###..", ".#####.", ".#####.", ".##.##.", "..###..");
sprite!(CLOUD, "..###..", ".#...##", "#.....#", "#.....#", ".#####.");
sprite!(SUN, "#..#..#", ".#.#.#.", "..###..", "#.###.#", "..###..", ".#.#.#.", "#..#..#");
sprite!(MOON, "..###..", ".##....", "##.....", "##.....", "##.....", ".##....", "..###..");
sprite!(EURO, "..####.", ".#....#", "####...", ".#.....", "####...", ".#....#", "..####.");
sprite!(NOTE, "...##..", "...#.#.", "...#...", "...#...", ".###...", "####...", ".##....");
sprite!(WIND, ".###...", "....#..", "######.", ".......", "..####.", "......#", ".#####.");
sprite!(SAT, "#.....#", "##...##", ".#####.", "..###..", ".#####.", "##...##", "#.....#");
sprite!(TROPHY, "#######", ".#####.", ".#####.", "..###..", "...#...", "..###..", ".#####.");
sprite!(FLAKE, "#..#..#", ".#.#.#.", "..###..", "#######", "..###..", ".#.#.#.", "#..#..#");
sprite!(FOG, ".......", "#####..", ".......", "..#####", ".......", "#####..", ".......");
sprite!(THERM, "...#...", "..#.#..", "..#.#..", "..#.#..", ".##.##.", ".#####.", "..###..");

/// Every icon with its name, for tests and the catalogue.
pub static ICONS: &[(&str, &Sprite)] = &[
    ("box", &BOX),
    ("server", &SERVER),
    ("branch", &BRANCH),
    ("rocket", &ROCKET),
    ("download", &DOWNLOAD),
    ("bolt", &BOLT),
    ("batlow", &BATLOW),
    ("bell", &BELL),
    ("bell2", &BELL2),
    ("check", &CHECK),
    ("tri", &TRI),
    ("cpu", &CPU),
    ("mem", &MEM),
    ("disk", &DISK),
    ("flame", &FLAME),
    ("cloud", &CLOUD),
    ("sun", &SUN),
    ("moon", &MOON),
    ("euro", &EURO),
    ("note", &NOTE),
    ("wind", &WIND),
    ("sat", &SAT),
    ("trophy", &TROPHY),
    ("flake", &FLAKE),
    ("fog", &FOG),
    ("therm", &THERM),
];

// ---- eye sprites, 7 wide, replace an eye ----
sprite!(XEYE, "##...##", ".##.##.", "..###..", "...#...", "..###..", ".##.##.", "##...##");
sprite!(STAR, "...#...", "...#...", "..###..", "#######", ".#####.", "..#.#..", ".#...#.");
sprite!(SPIRAL1, ".#####.", "#.....#", "#.###.#", "#.#.#.#", "#.#.###", "#.#....", ".####..");
sprite!(SPIRAL2, ".####..", "#.#....", "#.#.###", "#.#.#.#", "#.###.#", "#.....#", ".#####.");
sprite!(EUROEYE, "..####.", ".#....#", "####...", ".#.....", "####...", ".#....#", "..####.");
sprite!(SHADE, "#######", "#######", "#######");
sprite!(SQZ_L, "#......", ".#.....", "..#....", "...#...", "..#....", ".#.....", "#......");
sprite!(SQZ_R, "......#", ".....#.", "....#..", "...#...", "....#..", ".....#.", "......#");
sprite!(ARC, "..###..", ".#...#.", "#.....#");

// ---- small things ----
sprite!(DOT, "#");
sprite!(COIN, "##", "##");
sprite!(Z, "##", ".#", "##");
sprite!(STARLET, "..#..", "..#..", "#####", ".###.", "#...#");

/// The disk icon with its bottom `n` rows filled solid, `n = 0..=7`.
pub static DISK_FILL: [Sprite; 8] = [
    Sprite {
        rows: &[
            ".#####.", "#.....#", ".#####.", "#.....#", ".#####.", "#.....#", ".#####.",
        ],
    },
    Sprite {
        rows: &[
            ".#####.", "#.....#", ".#####.", "#.....#", ".#####.", "#.....#", "#######",
        ],
    },
    Sprite {
        rows: &[
            ".#####.", "#.....#", ".#####.", "#.....#", ".#####.", "#######", "#######",
        ],
    },
    Sprite {
        rows: &[
            ".#####.", "#.....#", ".#####.", "#.....#", "#######", "#######", "#######",
        ],
    },
    Sprite {
        rows: &[
            ".#####.", "#.....#", ".#####.", "#######", "#######", "#######", "#######",
        ],
    },
    Sprite {
        rows: &[
            ".#####.", "#.....#", "#######", "#######", "#######", "#######", "#######",
        ],
    },
    Sprite {
        rows: &[
            ".#####.", "#######", "#######", "#######", "#######", "#######", "#######",
        ],
    },
    Sprite {
        rows: &[
            "#######", "#######", "#######", "#######", "#######", "#######", "#######",
        ],
    },
];

// ---- tints per source ----
pub const T_GITHUB: Color = Color::hex(0xe6e6ff);
pub const T_ARGO: Color = Color::hex(0xff9a3c);
pub const T_TORRENT: Color = Color::hex(0x5ac8fa);
pub const T_K8S: Color = Color::hex(0x4c8dff);
pub const T_UPS: Color = Color::hex(0xffe66d);
pub const T_LONGHORN: Color = Color::hex(0x8be9a5);
pub const T_PROM: Color = Color::hex(0xff6b57);
pub const T_ALERT: Color = Color::hex(0xff5a5a);
pub const T_ISS: Color = Color::hex(0xc9d6ff);
pub const T_SKY: Color = Color::hex(0xffd36b);
pub const T_PRICE: Color = Color::hex(0x7ee0c3);
pub const T_MEM: Color = Color::hex(0xb48cff);
pub const T_WX: Color = Color::hex(0x9ad4ff);
pub const T_SNOW: Color = Color::hex(0xe8f4ff);
pub const T_WIND: Color = Color::hex(0xb8c4cc);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_sprite_is_rectangular_and_icons_are_seven_wide() {
        for (name, s) in ICONS {
            assert_eq!(s.w(), 7, "{name} is not 7 wide");
            assert!(s.h() >= 5 && s.h() <= 7, "{name} height");
            for r in s.rows {
                assert_eq!(r.len(), 7, "{name} has a ragged row");
            }
        }
        for s in [
            &XEYE, &STAR, &SPIRAL1, &SPIRAL2, &EUROEYE, &SHADE, &SQZ_L, &SQZ_R, &ARC,
        ] {
            assert_eq!(s.w(), 7);
            for r in s.rows {
                assert_eq!(r.len(), 7);
            }
        }
        assert_eq!((DOT.w(), DOT.h()), (1, 1));
        assert_eq!((COIN.w(), COIN.h()), (2, 2));
        assert_eq!((Z.w(), Z.h()), (2, 3));
        assert_eq!((STARLET.w(), STARLET.h()), (5, 5));
        assert!(DOT.on(0, 0));
        assert!(!BOX.on(0, 0) && BOX.on(2, 0));
        assert!(!BOX.on(9, 9), "out of range is dark");
    }

    #[test]
    fn sqz_is_mirrored_and_spiral2_is_spiral1_upside_down() {
        for j in 0..7 {
            for i in 0..7 {
                assert_eq!(SQZ_L.on(i, j), SQZ_R.on(6 - i, j));
                assert_eq!(SPIRAL1.on(i, j), SPIRAL2.on(i, 6 - j));
            }
        }
    }

    #[test]
    fn disk_fill_levels_fill_from_the_bottom() {
        assert_eq!(DISK_FILL[0].rows, DISK.rows);
        assert!(DISK_FILL[7].rows.iter().all(|r| r == &"#######"));
        assert!(DISK_FILL[3].on(3, 6) && DISK_FILL[3].on(3, 4));
        assert!(!DISK_FILL[3].on(3, 3));
    }
}
