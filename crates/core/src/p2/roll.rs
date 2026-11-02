//! Rolling digits: when the formatted hero changes, only the changed
//! characters roll, staggered from the left, each keeping its old glyph until
//! its own roll starts.

use crate::anim::{Easing, Secs};

pub const ROLL_SECS: Secs = 0.5;
pub const ROLL_STAGGER_SECS: Secs = 0.03;
/// Travel of a glyph during its roll, in units of the font size.
pub const ROLL_TRAVEL: f32 = 0.82;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cell {
    pub cur: char,
    pub prev: Option<char>,
    pub start: Secs,
    pub up: bool,
}

/// One glyph to draw for a cell: `dy` is in units of the font size, positive
/// down; a rolling cell yields two glyphs (old then new).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RollGlyph {
    pub cell: usize,
    pub ch: char,
    pub dy: f32,
    pub alpha: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Roll {
    text: String,
    value: f32,
    cells: Vec<Cell>,
}

fn number_of(text: &str) -> f32 {
    let digits: String = text
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.' || *c == '-')
        .collect();
    digits.parse().unwrap_or(0.0)
}

impl Roll {
    pub fn new(text: &str, now: Secs) -> Roll {
        Roll {
            text: text.to_string(),
            value: number_of(text),
            cells: text
                .chars()
                .map(|c| Cell {
                    cur: c,
                    prev: None,
                    start: now - ROLL_SECS,
                    up: true,
                })
                .collect(),
        }
    }

    /// Show `text`: a length change resets, otherwise the changed cells roll,
    /// staggered 30 ms per cell from the left, up when the value increased.
    pub fn set(&mut self, text: &str, now: Secs) {
        if text == self.text {
            return;
        }
        if text.chars().count() != self.cells.len() {
            *self = Roll::new(text, now);
            return;
        }
        let value = number_of(text);
        let up = value >= self.value;
        for (i, (cell, ch)) in self.cells.iter_mut().zip(text.chars()).enumerate() {
            if cell.cur != ch {
                cell.prev = Some(cell.cur);
                cell.cur = ch;
                cell.start = now + i as Secs * ROLL_STAGGER_SECS;
                cell.up = up;
            }
        }
        self.text = text.to_string();
        self.value = value;
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn cells(&self) -> &[Cell] {
        &self.cells
    }

    pub fn rolling(&self, now: Secs) -> bool {
        self.cells
            .iter()
            .any(|c| c.prev.is_some() && now < c.start + ROLL_SECS)
    }

    pub fn glyphs(&self, now: Secs) -> Vec<RollGlyph> {
        let mut out = Vec::with_capacity(self.cells.len() * 2);
        for (i, c) in self.cells.iter().enumerate() {
            let k = ((now - c.start) / ROLL_SECS) as f32;
            match c.prev {
                Some(p) if k < 0.0 => out.push(RollGlyph {
                    cell: i,
                    ch: p,
                    dy: 0.0,
                    alpha: 1.0,
                }),
                Some(p) if k < 1.0 => {
                    let e = Easing::OutQuart.apply(k);
                    let dir = if c.up { 1.0 } else { -1.0 };
                    out.push(RollGlyph {
                        cell: i,
                        ch: p,
                        dy: -dir * e * ROLL_TRAVEL,
                        alpha: 1.0 - e,
                    });
                    out.push(RollGlyph {
                        cell: i,
                        ch: c.cur,
                        dy: dir * (1.0 - e) * ROLL_TRAVEL,
                        alpha: e,
                    });
                }
                _ => out.push(RollGlyph {
                    cell: i,
                    ch: c.cur,
                    dy: 0.0,
                    alpha: 1.0,
                }),
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn glyphs_of(r: &Roll, now: f64, cell: usize) -> Vec<RollGlyph> {
        r.glyphs(now)
            .into_iter()
            .filter(|g| g.cell == cell)
            .collect()
    }

    #[test]
    fn a_staggered_cell_shows_its_old_glyph_until_its_start() {
        let mut r = Roll::new("40", 0.0);
        r.set("52", 10.0);
        // cell 1 starts at 10.03: before that it still shows '0', not '2'
        let late = glyphs_of(&r, 10.01, 1);
        assert_eq!(late.len(), 1);
        assert_eq!((late[0].ch, late[0].dy, late[0].alpha), ('0', 0.0, 1.0));
        let early = glyphs_of(&r, 10.01, 0);
        assert_eq!(early.len(), 2, "cell 0 is rolling: old and new glyph");
        assert_eq!(early[0].ch, '4');
        assert_eq!(early[1].ch, '5');
        assert!(r.rolling(10.2));
        assert!(!r.rolling(10.6));
        assert_eq!(
            glyphs_of(&r, 10.6, 1),
            vec![RollGlyph {
                cell: 1,
                ch: '2',
                dy: 0.0,
                alpha: 1.0
            }]
        );
    }

    #[test]
    fn only_changed_characters_roll_and_direction_follows_the_value() {
        let mut r = Roll::new("42", 0.0);
        r.set("47", 1.0);
        assert_eq!(glyphs_of(&r, 1.1, 0).len(), 1, "the 4 stays put");
        let up = glyphs_of(&r, 1.1, 1);
        assert_eq!(up.len(), 2);
        assert!(up[0].dy < 0.0, "old leaves upward when increasing");
        assert!(up[1].dy > 0.0, "new enters from below");
        assert!(up[0].alpha < 1.0 && up[1].alpha > 0.0);
        let mut d = Roll::new("47", 0.0);
        d.set("42", 1.0);
        let down = glyphs_of(&d, 1.1, 1);
        assert!(
            down[0].dy > 0.0 && down[1].dy < 0.0,
            "mirrored when decreasing"
        );
        assert!(
            (down[1].dy.abs()
                - (ROLL_TRAVEL
                    * (1.0 - Easing::OutQuart.apply(((1.1 - 1.03) / ROLL_SECS) as f32))))
            .abs()
                < 1e-4
        );
    }

    #[test]
    fn a_length_change_resets_without_rolling() {
        let mut r = Roll::new("9", 0.0);
        r.set("10", 1.0);
        assert_eq!(r.text(), "10");
        assert_eq!(r.glyphs(1.1).len(), 2);
        assert!(!r.rolling(1.1));
        r.set("10", 2.0);
        assert!(!r.rolling(2.1), "same text is a no-op");
        let mut t = Roll::new("14:32", 0.0);
        t.set("14:31", 5.0);
        assert!(t.rolling(5.2));
        assert_eq!(glyphs_of(&t, 5.2, 2).len(), 1, "the colon never rolls");
    }
}
