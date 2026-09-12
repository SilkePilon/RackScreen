//! The acts: one short choreography per event, all sharing one envelope so any
//! act can follow any other. Bodies are a port of the approved mockup
//! (`acts-4.html`); `q` is body progress, `t` seconds since the act started.

use std::f32::consts::PI;

use crate::face_expr::{bump, ease, inseg, lerp, out_back, seg, Expr};
use crate::face_sprites::*;
use crate::mood::Mood;
use crate::theme::Color;

/// Envelope in and out, seconds.
pub const IN_S: f32 = 0.3;
/// Icon slot: 7×7 cells, left column and top row when fully in.
pub const SLOT_X: f32 = 9.0;
pub const SLOT_Y: f32 = 17.0;
/// How far the face rises while an icon is in the slot.
pub const RISE_PX: f32 = 30.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ActKind {
    // pods
    OhHi,
    Ouch,
    Bye,
    // nodes
    LostOne,
    ItsBack,
    // github
    Catch,
    StarryEyes,
    Merge,
    Party,
    Ding,
    EyeRoll,
    LevelUp,
}

impl ActKind {
    pub const ALL: &'static [ActKind] = &[
        ActKind::OhHi,
        ActKind::Ouch,
        ActKind::Bye,
        ActKind::LostOne,
        ActKind::ItsBack,
        ActKind::Catch,
        ActKind::StarryEyes,
        ActKind::Merge,
        ActKind::Party,
        ActKind::Ding,
        ActKind::EyeRoll,
        ActKind::LevelUp,
    ];

    /// Jumps the queue.
    pub fn is_severe(self) -> bool {
        matches!(self, ActKind::Ouch | ActKind::LostOne)
    }
    /// Habits never queue and any event interrupts them.
    pub fn is_habit(self) -> bool {
        false
    }
    /// At most one per 20 s.
    pub fn is_rate_limited(self) -> bool {
        matches!(self, ActKind::OhHi | ActKind::Bye)
    }

    pub fn def(self) -> ActDef {
        let d = |dur: f32,
                 mood: Mood,
                 icon: Option<&'static Sprite>,
                 tint: Option<Color>,
                 body: fn(&mut Frame)| ActDef {
            kind: self,
            dur,
            mood,
            icon,
            tint,
            rise: icon.is_some(),
            body,
        };
        match self {
            ActKind::OhHi => d(1.8, Mood::Content, Some(&BOX), Some(T_K8S), oh_hi),
            ActKind::Ouch => ActDef {
                rise: true,
                ..d(2.8, Mood::Worried, None, None, ouch)
            },
            ActKind::Bye => d(2.4, Mood::Content, Some(&BOX), Some(T_K8S), bye),
            ActKind::LostOne => d(3.0, Mood::Sad, Some(&SERVER), Some(T_K8S), lost_one),
            ActKind::ItsBack => d(2.6, Mood::Happy, Some(&SERVER), Some(T_K8S), its_back),
            ActKind::Catch => d(2.8, Mood::Excited, Some(&BRANCH), Some(T_GITHUB), catch),
            ActKind::StarryEyes => d(
                2.6,
                Mood::Excited,
                Some(&BRANCH),
                Some(T_GITHUB),
                starry_eyes,
            ),
            ActKind::Merge => d(2.6, Mood::Happy, Some(&BRANCH), Some(T_GITHUB), merge),
            ActKind::Party => d(3.2, Mood::Excited, Some(&BRANCH), Some(T_GITHUB), party),
            ActKind::Ding => d(2.0, Mood::Happy, Some(&CHECK), Some(T_LONGHORN), ding),
            ActKind::EyeRoll => d(2.6, Mood::Worried, Some(&CROSS), Some(T_ALERT), eye_roll),
            ActKind::LevelUp => d(3.0, Mood::Excited, Some(&TROPHY), Some(T_SKY), level_up),
        }
    }
}

pub type EyePair = [&'static Sprite; 2];
pub fn both(s: &'static Sprite) -> EyePair {
    [s, s]
}

/// Per-eye override an act may set; `[left, right]`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EyeOv {
    pub open: Option<f32>,
    pub scale: Option<f32>,
    pub tilt: Option<f32>,
}

/// Whole-matrix overlays the sampler applies; the `f32` is the act clock.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Overlay {
    None,
    Confetti(f32),
    FogBand(f32),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Post {
    /// Brightness multiplier for every lit cell (3.0 is lightning).
    pub flicker: f32,
    /// Minimum brightness for cells more than 104 px from the centre.
    pub rim: f32,
    pub overlay: Overlay,
}

impl Default for Post {
    fn default() -> Self {
        Post {
            flicker: 1.0,
            rim: 0.0,
            overlay: Overlay::None,
        }
    }
}

/// A sprite at a (fractional) cell position, in grid space, not rotated with the face.
#[derive(Clone, Debug, PartialEq)]
pub struct Placed {
    pub sprite: &'static Sprite,
    pub x: f32,
    pub y: f32,
    pub tint: Option<Color>,
    pub alpha: f32,
}

/// What one act frame asks the face to be. `e`, `off`, `rot`, `gaze` and
/// `eyes` shape the eyes; `placed` are sprites on top; `post` is applied to
/// the whole matrix.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub q: f32,
    pub t: f32,
    pub e: Expr,
    pub off: (f32, f32),
    pub rot: f32,
    pub gaze: (f32, f32),
    pub eyes: [EyeOv; 2],
    pub sprite: Option<EyePair>,
    pub icon_alpha: f32,
    pub icon_frame: Option<&'static Sprite>,
    pub placed: Vec<Placed>,
    pub post: Post,
    pub slot_y: f32,
}

impl Frame {
    pub fn at(&mut self, s: &'static Sprite, x: f32, y: f32, tint: Option<Color>, alpha: f32) {
        if alpha > 0.0 {
            self.placed.push(Placed {
                sprite: s,
                x,
                y,
                tint,
                alpha,
            });
        }
    }
    /// Relative to the icon slot.
    pub fn over(&mut self, s: &'static Sprite, dx: f32, dy: f32, tint: Option<Color>, alpha: f32) {
        let y = self.slot_y + dy;
        self.at(s, SLOT_X + dx, y, tint, alpha);
    }
    pub fn dot(&mut self, x: f32, y: f32, tint: Option<Color>, alpha: f32) {
        self.at(&DOT, x, y, tint, alpha);
    }
    /// Blend the expression toward `e` by `k`.
    pub fn set(&mut self, e: &Expr, k: f32) {
        self.e = Expr::lerp(self.e, *e, k);
    }
    /// Blend toward `e` as `q` runs from `a` to `b`.
    pub fn hold(&mut self, e: &Expr, a: f32, b: f32) {
        let k = seg(self.q, a, b);
        self.e = Expr::lerp(self.e, *e, k);
    }
}

pub struct ActDef {
    pub kind: ActKind,
    pub dur: f32,
    pub mood: Mood,
    pub icon: Option<&'static Sprite>,
    pub tint: Option<Color>,
    pub rise: bool,
    pub body: fn(&mut Frame),
}

/// Where in the envelope `p` (0..1 of `dur`) sits.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Env {
    /// 0 → 1 over the in phase, 1 → 0 over the out phase.
    pub env: f32,
    /// Body progress between the two.
    pub q: f32,
    pub inn: f32,
    pub out: f32,
}

pub fn envelope(p: f32, dur: f32) -> Env {
    let a = IN_S / dur;
    let inn = seg(p, 0.0, a);
    let out = seg(p, 1.0 - a, 1.0);
    Env {
        env: inn.min(1.0 - out),
        q: seg(p, a, 1.0 - a),
        inn,
        out,
    }
}

/// One frame of `def` at progress `p`, starting from the expression `held`
/// (the rest pose before the act) and the idle gaze. At `p = 0` and `p = 1`
/// the result is at rest: no offset, rotation, sprite or visible icon.
pub fn run_act(def: &ActDef, p: f32, held: Expr, idle_gaze: (f32, f32)) -> Frame {
    let env = envelope(p, def.dur);
    let slot_y = SLOT_Y + (1.0 - env.env) * 8.0;
    let base_e = Expr::lerp(held, Expr::CONTENT, env.inn);
    let base_off = (0.0, if def.rise { -RISE_PX * env.env } else { 0.0 });
    let base_gaze = (
        idle_gaze.0 * (1.0 - env.env),
        lerp(idle_gaze.1, 0.9, env.env),
    );
    let mut f = Frame {
        q: env.q,
        t: p * def.dur,
        e: base_e,
        off: base_off,
        rot: 0.0,
        gaze: base_gaze,
        eyes: [EyeOv::default(); 2],
        sprite: None,
        icon_alpha: 1.0,
        icon_frame: None,
        placed: Vec::new(),
        post: Post::default(),
        slot_y,
    };
    (def.body)(&mut f);
    // in phase: fade the body's contribution in, so p = 0 is the rest pose
    let k = env.inn;
    f.e = Expr::lerp(base_e, f.e, k);
    f.off = (lerp(base_off.0, f.off.0, k), lerp(base_off.1, f.off.1, k));
    f.rot *= k;
    f.gaze = (
        lerp(base_gaze.0, f.gaze.0, k),
        lerp(base_gaze.1, f.gaze.1, k),
    );
    f.post.flicker = lerp(1.0, f.post.flicker, k);
    f.post.rim *= k;
    for pl in &mut f.placed {
        pl.alpha *= k;
    }
    if k < 0.5 {
        f.sprite = None;
        f.eyes = [EyeOv::default(); 2];
        f.post.overlay = Overlay::None;
    }
    if let Some(icon) = def.icon {
        f.placed.insert(
            0,
            Placed {
                sprite: f.icon_frame.unwrap_or(icon),
                x: SLOT_X,
                y: slot_y,
                tint: def.tint,
                alpha: f.icon_alpha,
            },
        );
    }
    // out phase: everything settles into the act's mood
    let o = env.out;
    f.e = Expr::lerp(f.e, Expr::of(def.mood), o);
    if o > 0.0 {
        f.sprite = None;
        f.eyes = [EyeOv::default(); 2];
        f.rot *= 1.0 - o;
        // decay the body's own offset and gaze; the envelope baseline (rise, look-down) already follows `env`
        f.off = (
            base_off.0 + (f.off.0 - base_off.0) * (1.0 - o),
            base_off.1 + (f.off.1 - base_off.1) * (1.0 - o),
        );
        f.gaze = (
            base_gaze.0 + (f.gaze.0 - base_gaze.0) * (1.0 - o),
            base_gaze.1 + (f.gaze.1 - base_gaze.1) * (1.0 - o),
        );
        f.post.flicker = lerp(f.post.flicker, 1.0, o);
        f.post.rim *= 1.0 - o;
        let first_is_icon = def.icon.is_some();
        for (i, pl) in f.placed.iter_mut().enumerate() {
            if !(first_is_icon && i == 0) {
                pl.alpha *= 1.0 - o;
            }
        }
        if o >= 0.5 {
            f.post.overlay = Overlay::None;
        }
    }
    f
}

fn blink(t: f32, hz: f32, low: f32) -> f32 {
    if (t * hz).sin() > 0.0 {
        1.0
    } else {
        low
    }
}

// ---------------- pods ----------------

fn oh_hi(f: &mut Frame) {
    if f.q < 0.5 {
        f.e.open = 1.25;
    }
}

fn ouch(f: &mut Frame) {
    let (q, t) = (f.q, f.t);
    let fall = seg(q, 0.0, 0.3);
    if q < 0.3 {
        f.at(&BOX, SLOT_X, -7.0 + fall * 12.0, Some(T_K8S), 1.0);
    }
    if inseg(q, 0.3, 0.42) {
        let b = bump(q, 0.3, 0.42);
        f.off.0 -= 6.0 * b;
        f.off.1 += 8.0 * b;
    }
    if inseg(q, 0.3, 0.85) {
        f.sprite = Some(both(&XEYE));
    }
    if q >= 0.3 {
        let s = seg(q, 0.3, 0.55);
        f.at(&BOX, SLOT_X, 5.0 + s * (SLOT_Y - 5.0), Some(T_K8S), 1.0);
        if s >= 1.0 {
            f.at(
                &CROSS,
                SLOT_X + 1.0,
                SLOT_Y + 1.0,
                Some(T_ALERT),
                blink(t, 12.0, 0.2),
            );
        }
    }
    f.hold(&Expr::WORRIED, 0.85, 1.0);
}

fn bye(f: &mut Frame) {
    let q = f.q;
    f.icon_alpha = 1.0 - seg(q, 0.3, 0.8);
    if inseg(q, 0.4, 0.55) {
        f.e.open = 0.08;
    }
    if inseg(q, 0.7, 0.95) {
        f.off.1 -= 6.0 * bump(q, 0.7, 0.95);
    }
}

// ---------------- nodes ----------------

fn lost_one(f: &mut Frame) {
    let (q, t) = (f.q, f.t);
    f.over(&CROSS, 1.0, 0.0, Some(T_ALERT), blink(t, 12.0, 0.15));
    f.hold(&Expr::SAD, 0.1, 0.4);
    if q > 0.4 {
        let d = (t * 0.7).fract();
        f.dot(5.0, 13.0 + d * 4.0, Some(T_TORRENT), 1.0 - d * 0.5);
    }
}

fn its_back(f: &mut Frame) {
    let q = f.q;
    f.over(
        &CHECK,
        0.0,
        -1.0,
        Some(T_LONGHORN),
        out_back(seg(q, 0.05, 0.3)),
    );
    f.hold(&Expr::HAPPY, 0.1, 0.3);
    if inseg(q, 0.3, 0.8) {
        f.off.1 -= (seg(q, 0.3, 0.8) * PI * 2.0).sin().abs() * 10.0;
    }
}

// ---------------- github ----------------

fn catch(f: &mut Frame) {
    let q = f.q;
    let s = seg(q, 0.05, 0.55);
    let gx = 26.0 - s * 14.5;
    let gy = 6.0 + (s * PI).sin() * -4.0 + s * 4.0;
    if s < 1.0 {
        f.dot(gx, gy, Some(T_GITHUB), 1.0);
        f.gaze = ((gx - 12.0) / 12.0, (gy - 9.0) / 10.0);
    }
    if inseg(q, 0.55, 0.7) {
        let b = bump(q, 0.55, 0.7);
        let fade = 1.0 - seg(q, 0.6, 0.7);
        for k in 0..4 {
            let a = k as f32 * 1.57;
            f.dot(
                11.5 + a.cos() * b * 2.0,
                10.0 + a.sin() * b * 2.0,
                Some(T_UPS),
                fade,
            );
        }
        f.e.open = 1.2;
    }
    f.hold(&Expr::HAPPY, 0.6, 0.75);
    f.hold(&Expr::EXCITED, 0.8, 1.0);
}

fn starry_eyes(f: &mut Frame) {
    let (q, t) = (f.q, f.t);
    if inseg(q, 0.05, 0.85) {
        f.sprite = Some(both(&STAR));
        for i in 0..6 {
            let a = i as f32 * 1.05 + q * 2.0;
            if (t * 9.0 + i as f32 * 2.0).sin() > 0.3 {
                f.dot(
                    11.5 + a.cos() * 10.0,
                    11.5 + a.sin() * 10.0,
                    Some(T_UPS),
                    1.0,
                );
            }
        }
    }
    f.hold(&Expr::EXCITED, 0.85, 1.0);
}

fn merge(f: &mut Frame) {
    let q = f.q;
    let s = ease(seg(q, 0.0, 0.5));
    if q < 0.5 {
        f.dot(1.0 + s * 10.0, 9.0, Some(T_GITHUB), 1.0);
        f.dot(22.0 - s * 10.0, 9.0, Some(T_GITHUB), 1.0);
    } else {
        f.at(&COIN, 11.0, 8.5, Some(T_GITHUB), 1.0 - seg(q, 0.8, 1.0));
    }
    f.gaze = (0.0, 0.2);
    if inseg(q, 0.5, 0.85) {
        f.off.1 += (seg(q, 0.5, 0.85) * PI * 4.0).sin() * 4.0;
    }
    f.hold(&Expr::HAPPY, 0.6, 0.85);
}

fn party(f: &mut Frame) {
    let (q, t) = (f.q, f.t);
    f.set(&Expr::WOW, seg(q, 0.0, 0.15));
    f.post.overlay = Overlay::Confetti(t);
    f.off.1 -= (t * 8.0).sin().abs() * 7.0 * (1.0 - seg(q, 0.8, 1.0));
    f.hold(&Expr::EXCITED, 0.8, 1.0);
}

fn ding(f: &mut Frame) {
    let q = f.q;
    f.post.rim = 0.6 * bump(q, 0.0, 0.3);
    if inseg(q, 0.2, 0.7) {
        f.eyes[0].open = Some(0.06);
        f.e.lower = 0.4;
    }
    f.hold(&Expr::HAPPY, 0.7, 1.0);
}

fn eye_roll(f: &mut Frame) {
    let q = f.q;
    f.gaze = (0.0, lerp(0.9, -1.2, seg(q, 0.1, 0.35)));
    if inseg(q, 0.1, 0.75) {
        f.e.open = 0.4;
        f.e.tilt = 0.4;
        f.e.lower = 0.3;
    }
    f.hold(&Expr::WORRIED, 0.75, 1.0);
}

fn level_up(f: &mut Frame) {
    let q = f.q;
    for k in 0..3 {
        let kk = k as f32;
        f.at(
            &STARLET,
            4.0 + kk * 6.0,
            0.0,
            Some(T_UPS),
            seg(q, 0.05 + kk * 0.12, 0.15 + kk * 0.12),
        );
    }
    f.gaze = (0.0, lerp(0.9, -0.8, seg(q, 0.05, 0.2)));
    f.hold(&Expr::HAPPY, 0.3, 0.45);
    f.hold(&Expr::EXCITED, 0.85, 1.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(kind: ActKind, p: f32) -> Frame {
        run_act(&kind.def(), p, Expr::SAD, (0.4, -0.3))
    }

    #[test]
    fn out_phase_face_and_icon_settle_together() {
        // half way through the out phase the rise and the icon slide are both half done
        let d = ActKind::OhHi.def();
        let p = 1.0 - (IN_S / 2.0) / d.dur;
        let f = run_act(&d, p, Expr::CONTENT, (0.4, -0.3));
        assert!(
            (f.off.1 + RISE_PX * 0.5).abs() < 1e-3,
            "rise half way out: {}",
            f.off.1
        );
        assert!(
            (f.placed[0].y - (SLOT_Y + 4.0)).abs() < 1e-3,
            "icon half way out: {}",
            f.placed[0].y
        );
        assert!(
            (f.gaze.1 - (0.9 + (-0.3 - 0.9) * 0.5)).abs() < 1e-3,
            "gaze half way back: {}",
            f.gaze.1
        );
        // and a quarter of the way in, the same proportions hold on the way up
        let p = (IN_S / 4.0) / d.dur;
        let f = run_act(&d, p, Expr::CONTENT, (0.4, -0.3));
        assert!(
            (f.off.1 + RISE_PX * 0.25).abs() < 1e-3,
            "rise a quarter in: {}",
            f.off.1
        );
    }

    #[test]
    fn every_act_starts_and_ends_at_rest() {
        for kind in ActKind::ALL {
            let d = kind.def();
            for p in [0.0, 1.0] {
                let f = frame(*kind, p);
                assert!(
                    f.off.0.abs() < 1e-3 && f.off.1.abs() < 1e-3,
                    "{kind:?} offset at p={p}: {:?}",
                    f.off
                );
                assert!(f.rot.abs() < 1e-3, "{kind:?} rot at p={p}");
                assert!(f.sprite.is_none(), "{kind:?} eye sprite at p={p}");
                assert_eq!(
                    f.eyes,
                    [EyeOv::default(); 2],
                    "{kind:?} eye override at p={p}"
                );
                for pl in &f.placed {
                    assert!(
                        pl.alpha < 1e-3 || pl.y >= 24.0,
                        "{kind:?} visible sprite at p={p}: {:?}",
                        (pl.x, pl.y, pl.alpha)
                    );
                }
                assert!(
                    (f.gaze.0 - 0.4).abs() < 1e-3 && (f.gaze.1 + 0.3).abs() < 1e-3,
                    "{kind:?} gaze at p={p}"
                );
                assert!(
                    (f.post.flicker - 1.0).abs() < 1e-3 && f.post.rim.abs() < 1e-3,
                    "{kind:?} post at p={p}"
                );
                assert_eq!(f.post.overlay, Overlay::None, "{kind:?} overlay at p={p}");
            }
            assert!(
                frame(*kind, 0.0).e.close_to(&Expr::SAD, 1e-3),
                "{kind:?} does not start from the held mood"
            );
            assert!(
                frame(*kind, 1.0).e.close_to(&Expr::of(d.mood), 1e-3),
                "{kind:?} does not end in its mood"
            );
            assert!(d.dur >= 1.4 && d.dur <= 3.6, "{kind:?} duration");
        }
    }

    #[test]
    fn icon_sits_in_the_slot_and_the_face_rises_mid_act() {
        let f = frame(ActKind::LostOne, 0.5);
        let icon = &f.placed[0];
        assert!(std::ptr::eq(icon.sprite, &SERVER));
        assert_eq!((icon.x, icon.y), (SLOT_X, SLOT_Y));
        assert_eq!(icon.tint, Some(T_K8S));
        assert!((f.off.1 + RISE_PX).abs() < 1e-3, "rise: {}", f.off.1);
        assert!(f.gaze.1 > 0.8, "looking down at the icon");
        // ouch has no icon but still rises (rise: true)
        let f = frame(ActKind::Ouch, 0.5);
        assert!(f.off.1 < 0.0);
    }

    #[test]
    fn envelope_shape() {
        let e = envelope(0.0, 3.0);
        assert_eq!((e.env, e.q, e.inn, e.out), (0.0, 0.0, 0.0, 0.0));
        let e = envelope(0.05, 3.0);
        assert!((e.env - 0.5).abs() < 1e-6 && e.q == 0.0);
        let e = envelope(0.5, 3.0);
        assert!((e.env - 1.0).abs() < 1e-6 && (e.q - 0.5).abs() < 1e-6);
        let e = envelope(0.95, 3.0);
        assert!((e.env - 0.5).abs() < 1e-6 && e.q == 1.0 && (e.out - 0.5).abs() < 1e-6);
    }

    #[test]
    fn ouch_shows_x_eyes_and_lands_the_box() {
        let d = ActKind::Ouch.def();
        let mid = run_act(&d, 0.55, Expr::CONTENT, (0.0, 0.0));
        assert!(mid.sprite.is_some_and(|s| std::ptr::eq(s[0], &XEYE)));
        let boxes: Vec<&Placed> = mid
            .placed
            .iter()
            .filter(|p| std::ptr::eq(p.sprite, &BOX))
            .collect();
        assert_eq!(boxes.len(), 1);
        assert!((boxes[0].y - SLOT_Y).abs() < 1e-3);
        assert_eq!(d.mood, Mood::Worried);
    }

    #[test]
    fn kinds_have_the_right_flags() {
        assert!(ActKind::Ouch.is_severe() && ActKind::LostOne.is_severe());
        assert!(!ActKind::Catch.is_severe());
        assert!(ActKind::OhHi.is_rate_limited() && ActKind::Bye.is_rate_limited());
        assert!(!ActKind::OhHi.is_habit());
        let kinds: std::collections::HashSet<ActKind> = ActKind::ALL.iter().copied().collect();
        assert_eq!(kinds.len(), ActKind::ALL.len(), "ALL has duplicates");
        for k in ActKind::ALL {
            assert_eq!(k.def().kind, *k);
        }
    }
}
