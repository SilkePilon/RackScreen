//! Transition styles: how one frame gives way to the next on a role change
//! and around a show. `composite` is pure pixel work on two RGBA frames, so
//! the render workers, the goldens and the web previews agree on the look.

use std::sync::OnceLock;

use crate::anim::{Easing, Secs};
use crate::screens::Half;
use crate::theme::layout::SIZE;

/// Shortest and longest transition a config may ask for.
pub const MIN_SECS: Secs = 0.15;
pub const MAX_SECS: Secs = 2.0;
/// Per-panel delay of a show transition, as a share of its length, so a
/// directional style sweeps across the rack instead of hitting four panels
/// at once.
pub const STAGGER: Secs = 0.25;
/// Width of the soft edge of wipes, irises, clocks and blinds, in pixels.
const SOFT_PX: f32 = 2.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Style {
    /// Dim to black, then brighten the new frame (the original look).
    FadeBlack,
    /// Blend straight from the old frame to the new one.
    Crossfade,
    /// No animation: the new frame replaces the old one half-way through.
    Cut,
    /// The new frame pushes the old one out.
    Slide,
    /// The new frame slides in over the old one.
    Cover,
    /// A soft edge sweeps across, revealing the new frame.
    Wipe,
    /// A circle grows from the centre (or shrinks into it).
    Iris,
    /// A radial sweep from 12 o'clock.
    Clock,
    /// Pixels dissolve from one frame to the other.
    Dissolve,
    /// The old frame scales away while the new one scales in.
    Zoom,
    /// Strips reveal the new frame.
    Blinds,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
    In,
    Out,
    Cw,
    Ccw,
    Horizontal,
    Vertical,
}

impl Style {
    pub const ALL: [Style; 11] = [
        Style::FadeBlack,
        Style::Crossfade,
        Style::Cut,
        Style::Slide,
        Style::Cover,
        Style::Wipe,
        Style::Iris,
        Style::Clock,
        Style::Dissolve,
        Style::Zoom,
        Style::Blinds,
    ];

    /// The config name.
    pub fn name(self) -> &'static str {
        match self {
            Style::FadeBlack => "fade-black",
            Style::Crossfade => "crossfade",
            Style::Cut => "cut",
            Style::Slide => "slide",
            Style::Cover => "cover",
            Style::Wipe => "wipe",
            Style::Iris => "iris",
            Style::Clock => "clock",
            Style::Dissolve => "dissolve",
            Style::Zoom => "zoom",
            Style::Blinds => "blinds",
        }
    }

    pub fn parse(s: &str) -> Option<Style> {
        Style::ALL.into_iter().find(|k| k.name() == s)
    }

    /// The directions this style takes; empty when it has none.
    pub fn directions(self) -> &'static [Direction] {
        use Direction::*;
        match self {
            Style::Slide | Style::Cover | Style::Wipe => &[Left, Right, Up, Down],
            Style::Iris | Style::Zoom => &[In, Out],
            Style::Clock => &[Cw, Ccw],
            Style::Blinds => &[Horizontal, Vertical],
            Style::FadeBlack | Style::Crossfade | Style::Cut | Style::Dissolve => &[],
        }
    }

    /// The direction a fresh pick of this style gets.
    pub fn default_direction(self) -> Direction {
        match self {
            Style::Iris => Direction::Out,
            Style::Zoom => Direction::In,
            _ => self
                .directions()
                .first()
                .copied()
                .unwrap_or(Direction::Left),
        }
    }

    /// Both frames are rendered and composited; the others show one scene
    /// at a time (with a brightness), which is what the pipeline did before
    /// styles existed and costs nothing extra.
    pub fn uses_both_frames(self) -> bool {
        !matches!(self, Style::FadeBlack | Style::Cut)
    }

    /// Changes every pixel every frame: each frame is a full SPI push.
    pub fn heavy(self) -> bool {
        matches!(
            self,
            Style::Crossfade | Style::Slide | Style::Cover | Style::Dissolve | Style::Zoom
        )
    }

    /// Across a show's four panels, start one panel after the other so the
    /// style flows along the rack; plain fades hit all panels together.
    pub fn staggers(self) -> bool {
        !matches!(self, Style::FadeBlack | Style::Crossfade | Style::Cut)
    }
}

impl Direction {
    pub const ALL: [Direction; 10] = [
        Direction::Left,
        Direction::Right,
        Direction::Up,
        Direction::Down,
        Direction::In,
        Direction::Out,
        Direction::Cw,
        Direction::Ccw,
        Direction::Horizontal,
        Direction::Vertical,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Direction::Left => "left",
            Direction::Right => "right",
            Direction::Up => "up",
            Direction::Down => "down",
            Direction::In => "in",
            Direction::Out => "out",
            Direction::Cw => "cw",
            Direction::Ccw => "ccw",
            Direction::Horizontal => "horizontal",
            Direction::Vertical => "vertical",
        }
    }

    pub fn parse(s: &str) -> Option<Direction> {
        Direction::ALL.into_iter().find(|d| d.name() == s)
    }
}

/// A style with its direction (the style's default when it has none).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Kind {
    pub style: Style,
    pub direction: Direction,
}

impl Default for Kind {
    fn default() -> Self {
        Kind::of(Style::FadeBlack)
    }
}

impl Kind {
    /// `style` with its default direction.
    pub fn of(style: Style) -> Kind {
        Kind {
            style,
            direction: style.default_direction(),
        }
    }

    /// `style` with `direction`, which must be one of the style's (ignored,
    /// and the default used, when the style has none).
    pub fn new(style: Style, direction: Direction) -> Result<Kind, String> {
        let dirs = style.directions();
        if dirs.is_empty() {
            return Ok(Kind::of(style));
        }
        if !dirs.contains(&direction) {
            let names: Vec<&str> = dirs.iter().map(|d| d.name()).collect();
            return Err(format!(
                "transition '{}' takes direction {} (got {})",
                style.name(),
                names.join(", "),
                direction.name()
            ));
        }
        Ok(Kind { style, direction })
    }

    /// The styles `random` picks from: everything but the cut, with every
    /// direction.
    pub fn pool() -> &'static [Kind] {
        static POOL: OnceLock<Vec<Kind>> = OnceLock::new();
        POOL.get_or_init(|| {
            let mut v = Vec::new();
            for s in Style::ALL {
                if s == Style::Cut {
                    continue;
                }
                if s.directions().is_empty() {
                    v.push(Kind::of(s));
                } else {
                    v.extend(s.directions().iter().map(|&d| Kind {
                        style: s,
                        direction: d,
                    }));
                }
            }
            v
        })
    }

    /// One of `pool` for a random `r`.
    pub fn random(r: u64) -> Kind {
        let pool = Kind::pool();
        pool[(r % pool.len() as u64) as usize]
    }

    /// For a single-scene style: which frame shows at `t` and how bright.
    pub fn single(self, t: f32) -> (Half, f32) {
        match self.style {
            Style::Cut => (if t < 0.5 { Half::Out } else { Half::In }, 1.0),
            _ => crate::screens::fade(t),
        }
    }

    /// When panel `pos` (of `panels`, left to right) starts, given `step`
    /// between panels: left to right, or right to left for a leftward style.
    pub fn panel_delay(self, pos: u32, panels: u32, step: Secs) -> Secs {
        if !self.style.staggers() || panels < 2 {
            return 0.0;
        }
        let i = if self.direction == Direction::Left {
            panels - 1 - pos.min(panels - 1)
        } else {
            pos.min(panels - 1)
        };
        i as Secs * step
    }
}

/// A transition as configured: its kind and how long it takes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spec {
    pub kind: Kind,
    pub secs: Secs,
}

impl Default for Spec {
    fn default() -> Self {
        Spec {
            kind: Kind::default(),
            secs: crate::screens::TRANSITION_SECS,
        }
    }
}

impl Spec {
    /// Per-panel delay of a show transition.
    pub fn stagger(&self) -> Secs {
        if self.kind.style.staggers() {
            self.secs * STAGGER
        } else {
            0.0
        }
    }
}

/// The transition settings: for role changes, for shows (`None` = the
/// same), and whether every transition picks a random kind instead.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Transitions {
    pub screens: Spec,
    pub shows: Option<Spec>,
    pub random: bool,
}

impl Transitions {
    /// The spec of the next role change; `r` feeds the random pick.
    pub fn for_screen(&self, r: u64) -> Spec {
        if self.random {
            Spec {
                kind: Kind::random(r),
                secs: self.screens.secs,
            }
        } else {
            self.screens
        }
    }

    /// The spec of the next show: the shows' own when set, else the
    /// screens' (random included).
    pub fn for_show(&self, r: u64) -> Spec {
        self.shows.unwrap_or_else(|| self.for_screen(r))
    }
}

// ------------------------------------------------------------- compositing

const W: usize = SIZE as usize;
const N: usize = W * W;
/// The panel is round: pixels beyond this radius are never seen.
const PANEL_R: f32 = SIZE as f32 / 2.0;
/// An iris fully open; a little past the rim so the soft edge clears it.
const IRIS_R: f32 = PANEL_R + SOFT_PX;
const BLIND_COUNT: usize = 8;
const BLIND_PX: usize = W / BLIND_COUNT;
/// Share of a blinds transition over which the strips start one by one.
const BLIND_STAGGER: f32 = 0.25;
/// Each pixel of a dissolve crossfades over this share of the transition.
const DISSOLVE_SOFT: f32 = 0.35;
/// Size of the dissolve's soft patches, and how much fine grain is mixed in.
const NOISE_CELL: usize = 20;
const NOISE_GRAIN: f32 = 0.35;
/// How far ahead of a cover's leading edge the old frame is shaded.
const COVER_SHADOW_PX: usize = 10;
const COVER_SHADOW: f32 = 0.45;

fn ease(t: f32) -> f32 {
    Easing::InOutCubic.apply(t)
}

fn px(frame: &[u8]) -> &[[u8; 4]] {
    let (chunks, _) = frame.as_chunks::<4>();
    debug_assert_eq!(chunks.len(), N);
    chunks
}

fn px_mut(frame: &mut [u8]) -> &mut [[u8; 4]] {
    let (chunks, _) = frame.as_chunks_mut::<4>();
    debug_assert_eq!(chunks.len(), N);
    chunks
}

/// `f` towards `t` by `a`/256.
#[inline]
fn mix(f: [u8; 4], t: [u8; 4], a: u32) -> [u8; 4] {
    let b = 256 - a;
    let m = |i: usize| ((f[i] as u32 * b + t[i] as u32 * a) >> 8) as u8;
    [m(0), m(1), m(2), m(3)]
}

/// Coverage of the new frame for a signed distance `d` (pixels past the
/// edge, into the new frame): a soft edge `SOFT_PX` wide.
#[inline]
fn edge(d: f32) -> u32 {
    ((d / SOFT_PX + 0.5).clamp(0.0, 1.0) * 256.0) as u32
}

/// `out = from` where `cov` is 0, `to` where it is 256, blended between.
fn blend_map(from: &[u8], to: &[u8], out: &mut [u8], mut cov: impl FnMut(usize, usize) -> u32) {
    let (f, t, o) = (px(from), px(to), px_mut(out));
    for y in 0..W {
        let row = y * W;
        for x in 0..W {
            let i = row + x;
            o[i] = match cov(x, y) {
                0 => f[i],
                256 => t[i],
                a => mix(f[i], t[i], a),
            };
        }
    }
}

/// Per pixel: its angle from 12 o'clock clockwise (0..65536) and its
/// distance from the centre in 1/256 px.
fn polar() -> &'static [(u16, u16)] {
    static T: OnceLock<Vec<(u16, u16)>> = OnceLock::new();
    T.get_or_init(|| {
        let c = W as f32 / 2.0;
        (0..N)
            .map(|i| {
                let dx = (i % W) as f32 + 0.5 - c;
                let dy = (i / W) as f32 + 0.5 - c;
                let mut a = dx.atan2(-dy);
                if a < 0.0 {
                    a += std::f32::consts::TAU;
                }
                let ang = ((a / std::f32::consts::TAU) * 65536.0) as u32;
                let r = ((dx * dx + dy * dy).sqrt() * 256.0).round() as u32;
                (ang.min(65535) as u16, r.min(65535) as u16)
            })
            .collect()
    })
}

/// A fixed per-pixel threshold for the dissolve: soft patches (a smooth
/// noise over `NOISE_CELL` px cells) with fine grain on top, so frames
/// dissolve in clouds rather than TV static.
fn noise() -> &'static [u8] {
    static T: OnceLock<Vec<u8>> = OnceLock::new();
    T.get_or_init(|| {
        let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
        let mut next = move || {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            (x >> 40) as f32 / (1u64 << 24) as f32
        };
        let cells = W.div_ceil(NOISE_CELL) + 1;
        let grid: Vec<f32> = (0..cells * cells).map(|_| next()).collect();
        let mut out = Vec::with_capacity(N);
        for y in 0..W {
            let (cy, fy) = (y / NOISE_CELL, (y % NOISE_CELL) as f32 / NOISE_CELL as f32);
            for x in 0..W {
                let (cx, fx) = (x / NOISE_CELL, (x % NOISE_CELL) as f32 / NOISE_CELL as f32);
                let g = |i: usize, j: usize| grid[j * cells + i];
                let top = g(cx, cy) * (1.0 - fx) + g(cx + 1, cy) * fx;
                let bottom = g(cx, cy + 1) * (1.0 - fx) + g(cx + 1, cy + 1) * fx;
                let smooth = top * (1.0 - fy) + bottom * fy;
                let v = smooth * (1.0 - NOISE_GRAIN) + next() * NOISE_GRAIN;
                out.push((v * 255.0).round().clamp(0.0, 255.0) as u8);
            }
        }
        out
    })
}

fn crossfade(from: &[u8], to: &[u8], out: &mut [u8], e: f32) {
    let a = (e * 256.0).round() as u32;
    let (f, t, o) = (px(from), px(to), px_mut(out));
    for i in 0..N {
        o[i] = mix(f[i], t[i], a);
    }
}

/// Rows of `src` from `y0` go to rows of `dst` from `dy0`, `rows` of them,
/// columns `x0..x0+cols` to `dx0..`.
fn copy_block(
    src: &[[u8; 4]],
    dst: &mut [[u8; 4]],
    (x0, y0): (usize, usize),
    (dx0, dy0): (usize, usize),
    cols: usize,
    rows: usize,
) {
    for r in 0..rows {
        let s = (y0 + r) * W + x0;
        let d = (dy0 + r) * W + dx0;
        dst[d..d + cols].copy_from_slice(&src[s..s + cols]);
    }
}

fn slide(from: &[u8], to: &[u8], out: &mut [u8], e: f32, dir: Direction) {
    let d = ((e * W as f32).round() as usize).min(W);
    let (f, t, o) = (px(from), px(to), px_mut(out));
    match dir {
        Direction::Left => {
            copy_block(f, o, (d, 0), (0, 0), W - d, W);
            copy_block(t, o, (0, 0), (W - d, 0), d, W);
        }
        Direction::Right => {
            copy_block(f, o, (0, 0), (d, 0), W - d, W);
            copy_block(t, o, (W - d, 0), (0, 0), d, W);
        }
        Direction::Up => {
            copy_block(f, o, (0, d), (0, 0), W, W - d);
            copy_block(t, o, (0, 0), (0, W - d), W, d);
        }
        Direction::Down => {
            copy_block(f, o, (0, 0), (0, d), W, W - d);
            copy_block(t, o, (0, W - d), (0, 0), W, d);
        }
        _ => unreachable!("slide takes left, right, up or down"),
    }
}

/// Darken `o[i]` by the cover shadow at `dist` px from the edge.
#[inline]
fn shade(p: [u8; 4], dist: usize) -> [u8; 4] {
    let k = (256.0 * (1.0 - COVER_SHADOW * (1.0 - dist as f32 / COVER_SHADOW_PX as f32))) as u32;
    [
        ((p[0] as u32 * k) >> 8) as u8,
        ((p[1] as u32 * k) >> 8) as u8,
        ((p[2] as u32 * k) >> 8) as u8,
        p[3],
    ]
}

fn cover(from: &[u8], to: &[u8], out: &mut [u8], e: f32, dir: Direction) {
    let d = ((e * W as f32).round() as usize).min(W);
    let (f, t, o) = (px(from), px(to), px_mut(out));
    o.copy_from_slice(f);
    if d == 0 {
        return;
    }
    match dir {
        Direction::Left => {
            // the new frame's left edge is at W - d
            let x = W - d;
            for s in 0..COVER_SHADOW_PX.min(x) {
                for y in 0..W {
                    let i = y * W + x - 1 - s;
                    o[i] = shade(o[i], s);
                }
            }
            copy_block(t, o, (0, 0), (x, 0), d, W);
        }
        Direction::Right => {
            // the new frame's right edge is at d
            for s in 0..COVER_SHADOW_PX.min(W - d) {
                for y in 0..W {
                    let i = y * W + d + s;
                    o[i] = shade(o[i], s);
                }
            }
            copy_block(t, o, (W - d, 0), (0, 0), d, W);
        }
        Direction::Up => {
            let y = W - d;
            for s in 0..COVER_SHADOW_PX.min(y) {
                let row = (y - 1 - s) * W;
                for p in &mut o[row..row + W] {
                    *p = shade(*p, s);
                }
            }
            copy_block(t, o, (0, 0), (0, y), W, d);
        }
        Direction::Down => {
            for s in 0..COVER_SHADOW_PX.min(W - d) {
                let row = (d + s) * W;
                for p in &mut o[row..row + W] {
                    *p = shade(*p, s);
                }
            }
            copy_block(t, o, (0, W - d), (0, 0), W, d);
        }
        _ => unreachable!("cover takes left, right, up or down"),
    }
}

fn wipe(from: &[u8], to: &[u8], out: &mut [u8], e: f32, dir: Direction) {
    // the edge travels the panel plus the soft edge, so both ends are clean
    let p = e * (W as f32 + 2.0 * SOFT_PX) - SOFT_PX;
    let cov: Vec<u32> = (0..W)
        .map(|v| {
            let v = v as f32 + 0.5;
            edge(match dir {
                Direction::Left | Direction::Up => v - (W as f32 - p),
                Direction::Right | Direction::Down => p - v,
                _ => unreachable!("wipe takes left, right, up or down"),
            })
        })
        .collect();
    match dir {
        Direction::Left | Direction::Right => blend_map(from, to, out, |x, _| cov[x]),
        _ => blend_map(from, to, out, |_, y| cov[y]),
    }
}

fn iris(from: &[u8], to: &[u8], out: &mut [u8], e: f32, dir: Direction) {
    let polar = polar();
    // corners beyond the rim are never seen: treat them as the rim, so they
    // follow it instead of trailing the (invisible) end of the animation
    let rim = PANEL_R + 0.5;
    let r = match dir {
        Direction::Out => e * IRIS_R,
        _ => (1.0 - e) * IRIS_R,
    };
    blend_map(from, to, out, |x, y| {
        let dist = (polar[y * W + x].1 as f32 / 256.0).min(rim);
        edge(match dir {
            Direction::Out => r - dist,
            _ => dist - r,
        })
    });
}

fn clock(from: &[u8], to: &[u8], out: &mut [u8], e: f32, dir: Direction) {
    let polar = polar();
    let s = e * std::f32::consts::TAU;
    let scale = std::f32::consts::TAU / 65536.0;
    blend_map(from, to, out, |x, y| {
        let (ang, r) = polar[y * W + x];
        let mut a = ang as f32 * scale;
        if dir == Direction::Ccw && a > 0.0 {
            a = std::f32::consts::TAU - a;
        }
        // the sweep line is `s`; the arc from it to this pixel is `d` px
        let d = (s - a) * (r as f32 / 256.0);
        edge(d)
    });
}

fn dissolve(from: &[u8], to: &[u8], out: &mut [u8], e: f32) {
    let noise = noise();
    let lead = e * (1.0 + DISSOLVE_SOFT);
    let (f, t, o) = (px(from), px(to), px_mut(out));
    for i in 0..N {
        let thr = noise[i] as f32 / 255.0;
        let a = (((lead - thr) / DISSOLVE_SOFT).clamp(0.0, 1.0) * 256.0) as u32;
        o[i] = match a {
            0 => f[i],
            256 => t[i],
            a => mix(f[i], t[i], a),
        };
    }
}

fn blinds(from: &[u8], to: &[u8], out: &mut [u8], e: f32, dir: Direction) {
    let span = BLIND_PX as f32 + 2.0 * SOFT_PX;
    let cov: Vec<u32> = (0..W)
        .map(|v| {
            let strip = v / BLIND_PX;
            let local = (v % BLIND_PX) as f32 + 0.5;
            let start = strip as f32 * BLIND_STAGGER / (BLIND_COUNT - 1) as f32;
            let p = ((e - start) / (1.0 - BLIND_STAGGER)).clamp(0.0, 1.0);
            edge(p * span - SOFT_PX - local)
        })
        .collect();
    match dir {
        Direction::Vertical => blend_map(from, to, out, |x, _| cov[x]),
        _ => blend_map(from, to, out, |_, y| cov[y]),
    }
}

/// Source coordinate of each output row/column when the frame is scaled by
/// `s` about its centre: the integer part and a 0..256 fraction.
fn scale_axis(s: f32) -> Vec<(i32, u32)> {
    let c = W as f32 / 2.0;
    (0..W)
        .map(|v| {
            let src = c + (v as f32 + 0.5 - c) / s - 0.5;
            let fixed = (src * 256.0).round() as i32;
            (fixed.div_euclid(256), fixed.rem_euclid(256) as u32)
        })
        .collect()
}

/// Bilinear sample of `src` scaled by `s` at output pixel `(x, y)`, black
/// outside the frame.
#[inline]
fn sample(src: &[[u8; 4]], axis: &[(i32, u32)], x: usize, y: usize) -> [u8; 4] {
    let (xi, fx) = axis[x];
    let (yi, fy) = axis[y];
    let tap = |xi: i32, yi: i32| -> [u8; 4] {
        if xi < 0 || yi < 0 || xi >= W as i32 || yi >= W as i32 {
            [0, 0, 0, 255]
        } else {
            src[yi as usize * W + xi as usize]
        }
    };
    let (p00, p10, p01, p11) = (
        tap(xi, yi),
        tap(xi + 1, yi),
        tap(xi, yi + 1),
        tap(xi + 1, yi + 1),
    );
    let (w00, w10, w01, w11) = (
        (256 - fx) * (256 - fy),
        fx * (256 - fy),
        (256 - fx) * fy,
        fx * fy,
    );
    let ch = |i: usize| {
        ((p00[i] as u32 * w00 + p10[i] as u32 * w10 + p01[i] as u32 * w01 + p11[i] as u32 * w11)
            >> 16) as u8
    };
    [ch(0), ch(1), ch(2), ch(3)]
}

fn zoom(from: &[u8], to: &[u8], out: &mut [u8], e: f32, dir: Direction) {
    let (sf, st) = match dir {
        Direction::In => (1.0 + 0.2 * e, 0.85 + 0.15 * e),
        _ => (1.0 - 0.15 * e, 1.2 - 0.2 * e),
    };
    let (af, at) = (scale_axis(sf), scale_axis(st));
    let a = (e * 256.0).round() as u32;
    let (f, t, o) = (px(from), px(to), px_mut(out));
    for y in 0..W {
        for x in 0..W {
            let pf = sample(f, &af, x, y);
            let pt = sample(t, &at, x, y);
            o[y * W + x] = mix(pf, pt, a);
        }
    }
}

/// `out` = the frame between `from` and `to` at `t` (0 = `from`, 1 = `to`)
/// for a two-frame `kind`. All three are `SIZE`×`SIZE` RGBA. A single-scene
/// kind (`!uses_both_frames`) is treated as a crossfade here.
pub fn composite(kind: Kind, from: &[u8], to: &[u8], out: &mut [u8], t: f32) {
    assert_eq!(from.len(), N * 4, "from: {}x{} RGBA", SIZE, SIZE);
    assert_eq!(to.len(), N * 4, "to: {}x{} RGBA", SIZE, SIZE);
    assert_eq!(out.len(), N * 4, "out: {}x{} RGBA", SIZE, SIZE);
    if t <= 0.0 {
        out.copy_from_slice(from);
        return;
    }
    if t >= 1.0 {
        out.copy_from_slice(to);
        return;
    }
    let e = ease(t);
    let d = kind.direction;
    match kind.style {
        Style::FadeBlack | Style::Crossfade | Style::Cut => crossfade(from, to, out, e),
        Style::Slide => slide(from, to, out, e, d),
        Style::Cover => cover(from, to, out, e, d),
        Style::Wipe => wipe(from, to, out, e, d),
        Style::Iris => iris(from, to, out, e, d),
        Style::Clock => clock(from, to, out, e, d),
        Style::Dissolve => dissolve(from, to, out, e),
        Style::Zoom => zoom(from, to, out, e, d),
        Style::Blinds => blinds(from, to, out, e, d),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A frame that is `v` everywhere, opaque.
    fn flat(v: u8) -> Vec<u8> {
        let mut f = vec![v; N * 4];
        for p in f.as_chunks_mut::<4>().0 {
            p[3] = 255;
        }
        f
    }

    /// A frame with a gradient, so sliding and scaling are visible.
    fn gradient() -> Vec<u8> {
        let mut f = vec![0u8; N * 4];
        for (i, p) in f.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            *p = [(i % W) as u8, (i / W) as u8, 128, 255];
        }
        f
    }

    /// Share of pixels (0..1) that equal `to` rather than `from`, with the
    /// rest blended: the mean per-pixel progress.
    fn coverage(from: &[u8], to: &[u8], out: &[u8]) -> f64 {
        let mut sum = 0.0;
        let mut n = 0;
        for i in 0..N * 4 {
            if i % 4 == 3 || from[i] == to[i] {
                continue;
            }
            sum += (out[i] as f64 - from[i] as f64) / (to[i] as f64 - from[i] as f64);
            n += 1;
        }
        sum / n as f64
    }

    #[test]
    fn names_round_trip() {
        for s in Style::ALL {
            assert_eq!(Style::parse(s.name()), Some(s));
        }
        for d in Direction::ALL {
            assert_eq!(Direction::parse(d.name()), Some(d));
        }
        assert_eq!(Style::parse("swoosh"), None);
        assert_eq!(Kind::default(), Kind::of(Style::FadeBlack));
        assert_eq!(Kind::of(Style::Slide).direction, Direction::Left);
        assert_eq!(Kind::of(Style::Iris).direction, Direction::Out);
        assert_eq!(Kind::of(Style::Zoom).direction, Direction::In);
        assert_eq!(Kind::of(Style::Clock).direction, Direction::Cw);
        assert_eq!(Kind::of(Style::Blinds).direction, Direction::Horizontal);
    }

    #[test]
    fn new_checks_the_direction() {
        assert!(Kind::new(Style::Slide, Direction::Up).is_ok());
        let err = Kind::new(Style::Slide, Direction::Cw).unwrap_err();
        assert!(err.contains("slide") && err.contains("cw"), "{err}");
        assert!(Kind::new(Style::Iris, Direction::Left).is_err());
        // no direction: anything goes, the default is used
        assert_eq!(
            Kind::new(Style::Crossfade, Direction::Ccw).unwrap(),
            Kind::of(Style::Crossfade)
        );
    }

    #[test]
    fn pool_has_every_style_but_cut_with_every_direction() {
        let pool = Kind::pool();
        assert!(pool.iter().all(|k| k.style != Style::Cut));
        for s in Style::ALL {
            if s == Style::Cut {
                continue;
            }
            let dirs: Vec<Direction> = pool
                .iter()
                .filter(|k| k.style == s)
                .map(|k| k.direction)
                .collect();
            if s.directions().is_empty() {
                assert_eq!(dirs, vec![s.default_direction()]);
            } else {
                assert_eq!(dirs, s.directions());
            }
        }
        assert_eq!(pool.len(), 1 + 1 + 4 + 4 + 4 + 2 + 2 + 1 + 2 + 2);
        let picks: std::collections::HashSet<Kind> = (0..1000).map(Kind::random).collect();
        assert_eq!(picks.len(), pool.len(), "every kind comes up");
    }

    #[test]
    fn single_scene_styles() {
        let fb = Kind::of(Style::FadeBlack);
        assert_eq!(fb.single(0.25), crate::screens::fade(0.25));
        assert_eq!(fb.single(0.75), crate::screens::fade(0.75));
        let cut = Kind::of(Style::Cut);
        assert_eq!(cut.single(0.0), (Half::Out, 1.0));
        assert_eq!(cut.single(0.49), (Half::Out, 1.0));
        assert_eq!(cut.single(0.5), (Half::In, 1.0));
        assert_eq!(cut.single(1.0), (Half::In, 1.0));
        assert!(!Style::FadeBlack.uses_both_frames());
        assert!(!Style::Cut.uses_both_frames());
        assert!(Style::ALL
            .iter()
            .filter(|s| !matches!(s, Style::FadeBlack | Style::Cut))
            .all(|s| s.uses_both_frames()));
    }

    #[test]
    fn panel_delay_flows_along_the_rack() {
        let right = Kind::new(Style::Wipe, Direction::Right).unwrap();
        assert_eq!(right.panel_delay(0, 4, 0.1), 0.0);
        assert!((right.panel_delay(3, 4, 0.1) - 0.3).abs() < 1e-9);
        let left = Kind::new(Style::Wipe, Direction::Left).unwrap();
        assert!((left.panel_delay(0, 4, 0.1) - 0.3).abs() < 1e-9);
        assert_eq!(left.panel_delay(3, 4, 0.1), 0.0);
        // up, irises, clocks: left to right too
        assert!((Kind::of(Style::Iris).panel_delay(2, 4, 0.1) - 0.2).abs() < 1e-9);
        // fades hit every panel together; one panel never waits
        assert_eq!(Kind::of(Style::FadeBlack).panel_delay(3, 4, 0.1), 0.0);
        assert_eq!(Kind::of(Style::Crossfade).panel_delay(3, 4, 0.1), 0.0);
        assert_eq!(right.panel_delay(0, 1, 0.1), 0.0);
        assert!((right.panel_delay(9, 4, 0.1) - 0.3).abs() < 1e-9, "clamped");
        let spec = Spec {
            kind: right,
            secs: 0.4,
        };
        assert!((spec.stagger() - 0.1).abs() < 1e-9);
        assert_eq!(Spec::default().stagger(), 0.0);
    }

    #[test]
    fn transitions_pick_per_target() {
        let t = Transitions {
            screens: Spec {
                kind: Kind::of(Style::Slide),
                secs: 0.5,
            },
            shows: None,
            random: false,
        };
        assert_eq!(t.for_screen(7), t.screens);
        assert_eq!(t.for_show(7), t.screens);
        let own = Spec {
            kind: Kind::of(Style::Wipe),
            secs: 0.3,
        };
        let t = Transitions {
            shows: Some(own),
            random: true,
            ..t
        };
        assert_eq!(t.for_show(7), own, "the shows' own, even with random");
        let picks: std::collections::HashSet<Kind> =
            (0..200).map(|r| t.for_screen(r).kind).collect();
        assert!(picks.len() > 5, "random picks vary: {picks:?}");
        assert!((0..200).all(|r| t.for_screen(r).secs == 0.5), "secs kept");
    }

    #[test]
    fn every_kind_starts_at_from_and_ends_at_to() {
        let from = gradient();
        let to = flat(200);
        let mut out = vec![0u8; N * 4];
        for &k in Kind::pool() {
            composite(k, &from, &to, &mut out, 0.0);
            assert_eq!(out, from, "{k:?} at 0");
            composite(k, &from, &to, &mut out, 1.0);
            assert_eq!(out, to, "{k:?} at 1");
            // near the ends: close to the end frames
            composite(k, &from, &to, &mut out, 0.02);
            assert!(coverage(&from, &to, &out) < 0.1, "{k:?} at 0.02");
            composite(k, &from, &to, &mut out, 0.98);
            assert!(coverage(&from, &to, &out) > 0.9, "{k:?} at 0.98");
        }
    }

    #[test]
    fn coverage_grows_monotonically() {
        let from = flat(0);
        let to = flat(255);
        let mut out = vec![0u8; N * 4];
        for &k in Kind::pool() {
            if k.style == Style::Zoom {
                // scaling moves pixels; the mean over black borders is not monotonic
                continue;
            }
            let mut last = -1.0;
            for i in 0..=40 {
                composite(k, &from, &to, &mut out, i as f32 / 40.0);
                let c = coverage(&from, &to, &out);
                assert!(c >= last - 1e-6, "{k:?}: {c} < {last} at step {i}");
                last = c;
            }
            // half way through, something of both frames shows
            composite(k, &from, &to, &mut out, 0.5);
            let c = coverage(&from, &to, &out);
            assert!(c > 0.2 && c < 0.8, "{k:?} at 0.5: {c}");
        }
    }

    #[test]
    fn directions_differ() {
        let from = gradient();
        let to = flat(10);
        let mut a = vec![0u8; N * 4];
        let mut b = vec![0u8; N * 4];
        for s in Style::ALL {
            let dirs = s.directions();
            for i in 0..dirs.len() {
                for j in i + 1..dirs.len() {
                    composite(Kind::new(s, dirs[i]).unwrap(), &from, &to, &mut a, 0.4);
                    composite(Kind::new(s, dirs[j]).unwrap(), &from, &to, &mut b, 0.4);
                    assert_ne!(a, b, "{s:?} {:?} vs {:?}", dirs[i], dirs[j]);
                }
            }
        }
    }

    #[test]
    fn slide_moves_pixels_whole() {
        let from = gradient();
        let to = flat(7);
        let mut out = vec![0u8; N * 4];
        // the slide moves by whole pixels: every output pixel is a copy
        let k = Kind::new(Style::Slide, Direction::Left).unwrap();
        composite(k, &from, &to, &mut out, 0.5);
        let d = (ease(0.5) * W as f32).round() as usize;
        let o = px(&out);
        let f = px(&from);
        assert_eq!(o[100 * W], f[100 * W + d], "shifted left by {d}");
        assert_eq!(
            o[100 * W + W - 1],
            [7, 7, 7, 255],
            "the new frame on the right"
        );
        // a wipe's soft edge is two pixels: beyond it the frames are whole
        let k = Kind::new(Style::Wipe, Direction::Right).unwrap();
        composite(k, &from, &to, &mut out, 0.5);
        let o = px(&out);
        assert_eq!(o[100 * W], [7, 7, 7, 255]);
        assert_eq!(o[100 * W + W - 1], f[100 * W + W - 1]);
    }

    #[test]
    fn iris_and_clock_respect_the_round_panel() {
        let from = flat(0);
        let to = flat(255);
        let mut out = vec![0u8; N * 4];
        let o = |out: &[u8], x: usize, y: usize| out[(y * W + x) * 4];
        // iris out at 0.5: the centre is new, the rim is old
        composite(Kind::of(Style::Iris), &from, &to, &mut out, 0.5);
        assert_eq!(o(&out, 120, 120), 255);
        assert_eq!(o(&out, 120, 2), 0);
        assert_eq!(o(&out, 2, 2), 0, "corners follow the rim");
        // iris in: the other way round
        let k = Kind::new(Style::Iris, Direction::In).unwrap();
        composite(k, &from, &to, &mut out, 0.5);
        assert_eq!(o(&out, 120, 120), 0);
        assert_eq!(o(&out, 120, 2), 255);
        // clock cw at 0.5: the right half is new, the left half old
        composite(Kind::of(Style::Clock), &from, &to, &mut out, 0.5);
        assert_eq!(o(&out, 200, 120), 255);
        assert_eq!(o(&out, 40, 120), 0);
        let k = Kind::new(Style::Clock, Direction::Ccw).unwrap();
        composite(k, &from, &to, &mut out, 0.5);
        assert_eq!(o(&out, 200, 120), 0);
        assert_eq!(o(&out, 40, 120), 255);
    }

    #[test]
    fn zoom_scales_about_the_centre() {
        let from = flat(0);
        let mut to = flat(0);
        // a white square in the middle of `to`
        for y in 100..140 {
            for x in 100..140 {
                to[(y * W + x) * 4..(y * W + x) * 4 + 3].copy_from_slice(&[255, 255, 255]);
            }
        }
        let mut out = vec![0u8; N * 4];
        let o = |out: &[u8], x: usize, y: usize| out[(y * W + x) * 4];
        // zoom out at half way: `to` is 1.1× and half blended in, so the
        // square (100..140) reaches 98..142
        let k = Kind::new(Style::Zoom, Direction::Out).unwrap();
        composite(k, &from, &to, &mut out, 0.5);
        assert!(o(&out, 99, 120) > 100, "wider: {}", o(&out, 99, 120));
        assert_eq!(o(&out, 96, 120), 0);
        assert_eq!(o(&out, 120, 60), 0);
        // zoom in: `to` is 0.925×, so the square is 101.5..138.5
        let k = Kind::new(Style::Zoom, Direction::In).unwrap();
        composite(k, &from, &to, &mut out, 0.5);
        assert!(o(&out, 100, 120) < 30, "narrower: {}", o(&out, 100, 120));
        assert!(o(&out, 120, 120) > 100);
    }

    /// `cargo test --release -p rackscreen-core -- --ignored bench --nocapture`
    #[test]
    #[ignore]
    fn bench_every_kind() {
        let from = gradient();
        let to = flat(200);
        let mut out = vec![0u8; N * 4];
        let _ = polar();
        let _ = noise();
        for &k in Kind::pool() {
            let t0 = std::time::Instant::now();
            let n = 200;
            for i in 0..n {
                composite(k, &from, &to, &mut out, (i % 50) as f32 / 50.0 + 0.01);
            }
            let per = t0.elapsed().as_secs_f64() * 1000.0 / n as f64;
            eprintln!(
                "{:>10} {:<10} {per:.3} ms",
                k.style.name(),
                k.direction.name()
            );
        }
    }
}
