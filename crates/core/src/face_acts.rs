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
    // argo cd
    Launch,
    Grump,
    Wink,
    // qbittorrent
    Incoming,
    GotIt,
    // ups
    LightsFlicker,
    Phew,
    OnFumes,
    // alerts
    Alarm,
    AllClear,
    // heat and load
    TooHot,
    WorkingHard,
    Stuffed,
    // longhorn
    Hmm,
    DisksFine,
    SoFull,
    // link
    Hello,
    FoundYou,
    // weather habits
    Sunny,
    Raining,
    Windy,
    Snowing,
    Foggy,
    MehClouds,
    Heatwave,
    Freezing,
    // sky and air events
    Lightning,
    UhOhRain,
    Cough,
    Morning,
    Evening,
    Awoo,
    LookUp,
    // prices
    KaChing,
    Expensive,
    // boot and idle habits
    WakeUp,
    Sneeze,
    Humming,
    Peek,
    Stretch,
    Scanning,
    Hic,
    DozingOff,
    Dizzy,
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
        ActKind::Launch,
        ActKind::Grump,
        ActKind::Wink,
        ActKind::Incoming,
        ActKind::GotIt,
        ActKind::LightsFlicker,
        ActKind::Phew,
        ActKind::OnFumes,
        ActKind::Alarm,
        ActKind::AllClear,
        ActKind::TooHot,
        ActKind::WorkingHard,
        ActKind::Stuffed,
        ActKind::Hmm,
        ActKind::DisksFine,
        ActKind::SoFull,
        ActKind::Hello,
        ActKind::FoundYou,
        ActKind::Sunny,
        ActKind::Raining,
        ActKind::Windy,
        ActKind::Snowing,
        ActKind::Foggy,
        ActKind::MehClouds,
        ActKind::Heatwave,
        ActKind::Freezing,
        ActKind::Lightning,
        ActKind::UhOhRain,
        ActKind::Cough,
        ActKind::Morning,
        ActKind::Evening,
        ActKind::Awoo,
        ActKind::LookUp,
        ActKind::KaChing,
        ActKind::Expensive,
        ActKind::WakeUp,
        ActKind::Sneeze,
        ActKind::Humming,
        ActKind::Peek,
        ActKind::Stretch,
        ActKind::Scanning,
        ActKind::Hic,
        ActKind::DozingOff,
        ActKind::Dizzy,
    ];

    /// Jumps the queue.
    pub fn is_severe(self) -> bool {
        matches!(
            self,
            ActKind::Ouch
                | ActKind::LostOne
                | ActKind::LightsFlicker
                | ActKind::Hello
                | ActKind::Lightning
        )
    }
    /// Habits never queue and any event interrupts them.
    pub fn is_habit(self) -> bool {
        matches!(
            self,
            ActKind::Sunny
                | ActKind::Raining
                | ActKind::Windy
                | ActKind::Snowing
                | ActKind::Foggy
                | ActKind::MehClouds
                | ActKind::Heatwave
                | ActKind::Freezing
                | ActKind::Sneeze
                | ActKind::Humming
                | ActKind::Peek
                | ActKind::Stretch
                | ActKind::Scanning
                | ActKind::Hic
                | ActKind::DozingOff
        )
    }
    /// One of the weighted idle habits picked when nothing is happening.
    pub fn is_idle_habit(self) -> bool {
        IDLE_HABITS.iter().any(|(k, _)| *k == self)
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
            ActKind::Launch => d(2.8, Mood::Happy, Some(&ROCKET), Some(T_ARGO), launch),
            ActKind::Grump => d(2.6, Mood::Angry, Some(&TRI), Some(T_ALERT), grump),
            ActKind::Wink => d(2.2, Mood::Happy, Some(&ROCKET), Some(T_ARGO), wink),
            ActKind::Incoming => d(
                2.8,
                Mood::Content,
                Some(&DOWNLOAD),
                Some(T_TORRENT),
                incoming,
            ),
            ActKind::GotIt => d(3.2, Mood::Happy, Some(&DOWNLOAD), Some(T_TORRENT), got_it),
            ActKind::LightsFlicker => {
                d(3.0, Mood::Scared, Some(&BOLT), Some(T_UPS), lights_flicker)
            }
            ActKind::Phew => d(2.8, Mood::Content, Some(&BOLT), Some(T_UPS), phew),
            ActKind::OnFumes => d(3.0, Mood::Worried, Some(&BATLOW), Some(T_UPS), on_fumes),
            ActKind::Alarm => d(3.0, Mood::Angry, Some(&BELL), Some(T_ALERT), alarm),
            ActKind::AllClear => d(2.6, Mood::Content, Some(&BELL), Some(T_ALERT), all_clear),
            ActKind::TooHot => d(3.0, Mood::Hot, Some(&FLAME), Some(T_PROM), too_hot),
            ActKind::WorkingHard => d(2.8, Mood::Hot, Some(&CPU), Some(T_PROM), working_hard),
            ActKind::Stuffed => d(2.8, Mood::Hot, Some(&MEM), Some(T_MEM), stuffed),
            ActKind::Hmm => d(2.8, Mood::Angry, Some(&DISK), Some(T_LONGHORN), hmm),
            ActKind::DisksFine => d(
                2.2,
                Mood::Content,
                Some(&DISK),
                Some(T_LONGHORN),
                disks_fine,
            ),
            ActKind::SoFull => d(3.0, Mood::Worried, Some(&DISK), Some(T_LONGHORN), so_full),
            ActKind::Hello => d(3.2, Mood::Worried, Some(&CLOUD), Some(T_K8S), hello),
            ActKind::FoundYou => d(2.2, Mood::Happy, Some(&CLOUD), Some(T_K8S), found_you),
            ActKind::Sunny => d(3.2, Mood::Content, Some(&SUN), Some(T_SKY), sunny),
            ActKind::Raining => d(3.2, Mood::Content, Some(&CLOUD), Some(T_WX), raining),
            ActKind::Windy => d(3.2, Mood::Content, Some(&WIND), Some(T_WIND), windy),
            ActKind::Snowing => d(3.6, Mood::Content, Some(&FLAKE), Some(T_SNOW), snowing),
            ActKind::Foggy => d(3.4, Mood::Content, Some(&FOG), Some(T_WIND), foggy),
            ActKind::MehClouds => d(3.2, Mood::Content, Some(&CLOUD), Some(T_WIND), meh_clouds),
            ActKind::Heatwave => d(3.2, Mood::Content, Some(&THERM), Some(T_PROM), heatwave),
            ActKind::Freezing => d(3.0, Mood::Content, Some(&FLAKE), Some(T_SNOW), freezing),
            ActKind::Lightning => d(3.0, Mood::Scared, Some(&BOLT), Some(T_WX), lightning),
            ActKind::UhOhRain => d(3.0, Mood::Content, Some(&CLOUD), Some(T_WX), uh_oh_rain),
            ActKind::Cough => d(2.6, Mood::Worried, Some(&WIND), Some(T_WIND), cough),
            ActKind::Morning => d(3.2, Mood::Content, Some(&SUN), Some(T_SKY), morning),
            ActKind::Evening => d(3.0, Mood::Content, Some(&MOON), Some(T_ISS), evening),
            ActKind::Awoo => d(3.2, Mood::Content, Some(&MOON), Some(T_ISS), awoo),
            ActKind::LookUp => d(3.4, Mood::Content, Some(&SAT), Some(T_ISS), look_up),
            ActKind::KaChing => d(2.8, Mood::Happy, Some(&EURO), Some(T_PRICE), ka_ching),
            ActKind::Expensive => d(2.8, Mood::Content, Some(&EURO), Some(T_PRICE), expensive),
            ActKind::WakeUp => d(3.6, Mood::Content, None, None, wake_up),
            ActKind::Sneeze => d(2.4, Mood::Content, None, None, sneeze),
            ActKind::Humming => d(3.0, Mood::Content, None, None, humming),
            ActKind::Peek => d(2.6, Mood::Content, None, None, peek),
            ActKind::Stretch => d(2.6, Mood::Content, None, None, stretch),
            ActKind::Scanning => d(3.4, Mood::Content, None, None, scanning),
            ActKind::Hic => d(1.4, Mood::Content, None, None, hic),
            ActKind::DozingOff => d(3.6, Mood::Sleepy, None, None, dozing_off),
            ActKind::Dizzy => d(3.2, Mood::Worried, None, None, dizzy),
        }
    }
}

/// Idle habits with their pick weights. `DozingOff` is only eligible while bored.
pub const IDLE_HABITS: &[(ActKind, u32)] = &[
    (ActKind::Sneeze, 1),
    (ActKind::Humming, 3),
    (ActKind::Peek, 3),
    (ActKind::Stretch, 3),
    (ActKind::Scanning, 3),
    (ActKind::Hic, 1),
    (ActKind::DozingOff, 4),
];

/// Current-conditions category from the WMO code, temperature and gusts.
/// Thunder (95..=99) is an event, so it returns `None`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeatherCat {
    Snow,
    Rain,
    Wind,
    Fog,
    Cold,
    Hot,
    Clouds,
    Sunny,
}

pub fn weather_cat(code: u16, temp_c: f32, gust_kmh: f32, is_day: bool) -> Option<WeatherCat> {
    if matches!(code, 71..=77 | 85 | 86) {
        return Some(WeatherCat::Snow);
    }
    if matches!(code, 51..=67 | 80..=82) {
        return Some(WeatherCat::Rain);
    }
    if gust_kmh >= 50.0 {
        return Some(WeatherCat::Wind);
    }
    if matches!(code, 45 | 48) {
        return Some(WeatherCat::Fog);
    }
    if temp_c <= 0.0 {
        return Some(WeatherCat::Cold);
    }
    if temp_c >= 28.0 {
        return Some(WeatherCat::Hot);
    }
    if matches!(code, 2 | 3) {
        return Some(WeatherCat::Clouds);
    }
    if matches!(code, 0 | 1) && is_day {
        return Some(WeatherCat::Sunny);
    }
    None
}

impl WeatherCat {
    pub fn act(self) -> ActKind {
        match self {
            WeatherCat::Snow => ActKind::Snowing,
            WeatherCat::Rain => ActKind::Raining,
            WeatherCat::Wind => ActKind::Windy,
            WeatherCat::Fog => ActKind::Foggy,
            WeatherCat::Cold => ActKind::Freezing,
            WeatherCat::Hot => ActKind::Heatwave,
            WeatherCat::Clouds => ActKind::MehClouds,
            WeatherCat::Sunny => ActKind::Sunny,
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

fn sweat(f: &mut Frame, gx: f32, speed: f32) {
    let d = (f.t * speed).fract();
    f.dot(gx, 1.0 + d * 6.0, Some(T_TORRENT), 1.0 - d * 0.3);
}

// ---------------- argo cd ----------------

fn launch(f: &mut Frame) {
    let (q, t) = (f.q, f.t);
    let s = ease(seg(q, 0.1, 0.7));
    f.icon_alpha = 0.0;
    let gy = f.slot_y - s * 26.0;
    f.at(&ROCKET, SLOT_X, gy, Some(T_ARGO), 1.0);
    for k in 1..4 {
        if (t * 25.0 + k as f32).sin() > 0.0 {
            let side = if k % 2 == 1 { -0.5 } else { 0.5 };
            f.dot(
                SLOT_X + 3.0 + side * (k as f32 - 1.0),
                gy + 7.0 + k as f32,
                Some(T_UPS),
                1.0,
            );
        }
    }
    f.gaze = (0.0, lerp(0.9, -1.0, s));
    if inseg(q, 0.1, 0.6) {
        f.e.open = 1.25;
    }
    f.hold(&Expr::HAPPY, 0.65, 0.9);
}

fn grump(f: &mut Frame) {
    let (q, t) = (f.q, f.t);
    f.icon_alpha = blink(t, 10.0, 0.15);
    f.hold(&Expr::ANGRY, 0.0, 0.2);
    if inseg(q, 0.3, 0.55) {
        f.off.0 += (seg(q, 0.3, 0.55) * PI * 3.0).sin() * 6.0;
    }
}

fn wink(f: &mut Frame) {
    let (q, t) = (f.q, f.t);
    f.icon_alpha = 0.7 + 0.3 * (t * 8.0).sin();
    if inseg(q, 0.1, 0.7) {
        f.eyes[1].open = Some(0.06);
        f.e.lower = 0.4;
    }
    f.hold(&Expr::HAPPY, 0.7, 1.0);
}

// ---------------- qbittorrent ----------------

fn incoming(f: &mut Frame) {
    let q = f.q;
    let n = (seg(q, 0.1, 0.9) * 12.0).floor() as usize;
    for k in 0..12 {
        f.dot(
            6.0 + k as f32,
            15.5,
            Some(T_TORRENT),
            if k < n { 1.0 } else { 0.25 },
        );
    }
    f.gaze = (-0.6 + seg(q, 0.1, 0.9) * 1.2, 0.9);
    f.set(&Expr::FOCUS, 1.0);
    f.off.1 += 3.0;
}

fn got_it(f: &mut Frame) {
    let q = f.q;
    let s = seg(q, 0.0, 0.35);
    f.at(
        &COIN,
        11.0,
        -2.0 + s * 11.0,
        Some(T_TORRENT),
        1.0 - seg(q, 0.75, 0.85),
    );
    f.gaze = if s < 1.0 {
        (0.0, -1.0 + s * 2.0)
    } else {
        (0.0, 0.1)
    };
    if inseg(q, 0.35, 0.7) {
        f.sprite = Some([&SQZ_L, &SQZ_R]);
        f.off.0 += (seg(q, 0.35, 0.7) * PI * 4.0).sin() * 4.0;
    }
    f.hold(&Expr::HAPPY, 0.7, 0.9);
}

// ---------------- ups ----------------

fn lights_flicker(f: &mut Frame) {
    let (q, t) = (f.q, f.t);
    if q < 0.3 {
        f.post.flicker = if (q * 60.0).sin() > -0.2 { 1.0 } else { 0.08 };
    }
    f.hold(&Expr::SCARED, 0.3, 0.5);
    if q > 0.45 {
        f.off.0 += (t * 70.0).sin() * 1.6;
    }
}

fn phew(f: &mut Frame) {
    let q = f.q;
    f.over(&CHECK, 1.0, 0.0, Some(T_LONGHORN), seg(q, 0.05, 0.2));
    f.hold(&Expr::RELIEF, 0.1, 0.3);
    if inseg(q, 0.3, 0.8) {
        f.off.1 += 6.0 * ease(seg(q, 0.3, 0.8));
    }
    f.hold(&Expr::CONTENT, 0.8, 1.0);
}

fn on_fumes(f: &mut Frame) {
    let (q, t) = (f.q, f.t);
    let on = blink(t, 8.0, 0.2);
    f.over(&DOT, 1.0, 2.0, Some(T_ALERT), on);
    f.over(&DOT, 1.0, 3.0, Some(T_ALERT), on);
    let s = seg(q, 0.1, 0.7);
    f.e.open = 1.0 - s * 0.7;
    f.off.1 += s * 6.0;
    if inseg(q, 0.6, 0.85) {
        f.e.open = 0.05 + 0.5 * (1.0 - bump(q, 0.6, 0.85));
    }
    f.hold(&Expr::WORRIED, 0.85, 1.0);
}

// ---------------- alerts ----------------

fn alarm(f: &mut Frame) {
    let (q, t) = (f.q, f.t);
    f.icon_frame = Some(if (t * 14.0).sin() > 0.0 {
        &BELL
    } else {
        &BELL2
    });
    f.post.rim = 0.45 + 0.45 * (t * 10.0).sin().max(0.0);
    f.set(&Expr::SCARED, seg(q, 0.0, 0.15));
    f.gaze = ((t * 5.0).sin() * 0.9, 0.3);
    f.hold(&Expr::ANGRY, 0.8, 1.0);
}

fn all_clear(f: &mut Frame) {
    let q = f.q;
    f.over(
        &CHECK,
        1.0,
        -1.0,
        Some(T_LONGHORN),
        out_back(seg(q, 0.05, 0.3)),
    );
    f.post.rim = 0.35 * bump(q, 0.1, 0.5);
    f.hold(&Expr::RELIEF, 0.2, 0.45);
    if inseg(q, 0.45, 0.8) {
        f.off.1 += 5.0 * bump(q, 0.45, 0.8);
    }
    f.hold(&Expr::CONTENT, 0.8, 1.0);
}

// ---------------- heat and load ----------------

fn too_hot(f: &mut Frame) {
    let (q, t) = (f.q, f.t);
    f.hold(&Expr::HOT, 0.0, 0.2);
    f.off.1 += (t * 12.0).sin().abs() * 3.0;
    if inseg(q, 0.4, 0.7) {
        f.off.0 += (seg(q, 0.4, 0.7) * PI * 4.0).sin() * 5.0;
    }
    sweat(f, 19.0, 0.8);
}

fn working_hard(f: &mut Frame) {
    let (q, t) = (f.q, f.t);
    f.hold(&Expr::GRIMACE, 0.0, 0.2);
    if inseg(q, 0.3, 0.8) {
        f.off.0 += (t * 40.0).sin() * 1.5;
    }
    sweat(f, 5.0, 0.9);
    f.hold(&Expr::HOT, 0.8, 1.0);
}

fn stuffed(f: &mut Frame) {
    let q = f.q;
    let s = seg(q, 0.1, 0.5);
    if q < 0.7 {
        f.e.open = 0.2;
        f.e.scale = 1.0 + 0.18 * s;
        f.e.sep = 1.0 + 0.12 * s;
        f.e.lower = 0.5;
    }
    if inseg(q, 0.5, 0.7) {
        let r = seg(q, 0.5, 0.7);
        for k in 0..3 {
            let kk = k as f32;
            f.dot(11.0 + kk * 0.5, 15.0 + r * 2.0 + kk, None, 1.0 - r);
        }
    }
    f.hold(&Expr::HOT, 0.75, 1.0);
}

// ---------------- longhorn ----------------

fn hmm(f: &mut Frame) {
    let (q, t) = (f.q, f.t);
    f.over(&CROSS, 1.0, 1.0, Some(T_ALERT), blink(t, 10.0, 0.15));
    f.hold(&Expr::GRIMACE, 0.0, 0.2);
    f.eyes[0].open = Some(0.4);
    f.eyes[1].open = Some(1.15);
    let s = ease(seg(q, 0.05, 0.35)) * (1.0 - seg(q, 0.8, 1.0));
    f.rot = -0.12 * s;
    f.off.0 -= 4.0 * s;
    f.hold(&Expr::ANGRY, 0.8, 1.0);
}

fn disks_fine(f: &mut Frame) {
    let q = f.q;
    f.over(
        &CHECK,
        1.0,
        0.0,
        Some(T_LONGHORN),
        out_back(seg(q, 0.05, 0.3)),
    );
    f.hold(&Expr::HAPPY, 0.1, 0.3);
    if inseg(q, 0.4, 0.65) {
        f.off.1 += 4.0 * bump(q, 0.4, 0.65);
    }
    f.hold(&Expr::CONTENT, 0.8, 1.0);
}

fn so_full(f: &mut Frame) {
    let q = f.q;
    let n = (seg(q, 0.05, 0.7) * 7.0).floor() as usize;
    f.icon_frame = Some(&DISK_FILL[n.min(7)]);
    let k = n as f32 / 7.0;
    f.e.scale = 1.0 + 0.16 * k;
    f.e.open = 1.1 + 0.1 * k;
    f.hold(&Expr::WORRIED, 0.8, 1.0);
}

// ---------------- link ----------------

fn hello(f: &mut Frame) {
    let (q, t) = (f.q, f.t);
    let on = (t * 8.0).sin() > 0.0;
    f.icon_alpha = if on { 0.9 } else { 0.3 };
    f.over(&CROSS, 1.0, 0.0, Some(T_ALERT), if on { 1.0 } else { 0.25 });
    f.e.open = 0.5;
    f.e.lower = 0.2;
    f.gaze = ((q * 9.0).sin() * 1.2, -0.2);
    f.hold(&Expr::WORRIED, 0.8, 1.0);
}

fn found_you(f: &mut Frame) {
    let q = f.q;
    f.icon_alpha = 0.3 + 0.7 * seg(q, 0.0, 0.2);
    let b = (seg(q, 0.1, 0.45) * PI * 2.0).sin();
    if b > 0.7 {
        f.e.open = 0.05;
    }
    if inseg(q, 0.45, 0.7) {
        f.e.open = 1.3;
    }
    f.hold(&Expr::HAPPY, 0.7, 0.95);
}

// ---------------- weather habits ----------------

fn sunny(f: &mut Frame) {
    let q = f.q;
    if inseg(q, 0.0, 0.35) {
        f.e.open = 0.3;
        f.e.lower = 0.3;
    }
    if inseg(q, 0.35, 0.9) {
        f.sprite = Some(both(&SHADE));
    }
    f.gaze = (0.0, lerp(0.9, 0.0, seg(q, 0.3, 0.4)));
    f.hold(&Expr::HAPPY, 0.9, 1.0);
}

fn raining(f: &mut Frame) {
    let t = f.t;
    for k in 0..7 {
        let kk = k as f32;
        let fr = (t * 1.2 + kk * 0.14).fract();
        f.dot(
            2.0 + kk * 3.2 + (k % 2) as f32,
            -1.0 + fr * 18.0,
            Some(T_WX),
            0.9 * (1.0 - fr * 0.3),
        );
    }
    f.gaze = (0.2, -1.0);
    f.e.open = 0.55 + if (t * 6.0).sin() > 0.6 { -0.45 } else { 0.0 };
}

fn windy(f: &mut Frame) {
    let (q, t) = (f.q, f.t);
    for k in 0..8 {
        let kk = k as f32;
        let fr = (t * 1.6 + kk * 0.125).fract();
        f.dot(
            24.0 - fr * 26.0,
            1.0 + ((k * 5) % 15) as f32 + (fr * 6.0).sin() * 0.6,
            Some(T_WIND),
            0.8,
        );
    }
    let s = seg(q, 0.0, 0.3) * (1.0 - seg(q, 0.85, 1.0));
    f.rot = 0.14 * s;
    f.off.0 -= 6.0 * s;
    f.e.open = 1.0 - 0.7 * s;
}

fn snowing(f: &mut Frame) {
    let (q, t) = (f.q, f.t);
    for k in 0..5 {
        let kk = k as f32;
        let fr = (t * 0.35 + kk / 5.0).fract();
        f.at(
            &COIN,
            1.0 + ((k * 7) % 20) as f32 + (fr * 7.0 + kk).sin() * 1.2,
            -2.0 + fr * 19.0,
            Some(T_SNOW),
            0.9,
        );
    }
    f.off.0 += (t * 45.0).sin() * 0.6;
    let fr = seg(q, 0.05, 0.4);
    if q < 0.8 {
        f.at(&COIN, 16.0, -2.0 + fr * 7.0, Some(T_SNOW), 1.0);
    }
    if inseg(q, 0.4, 0.75) {
        f.gaze = (0.5, -1.0);
        f.eyes[0] = EyeOv {
            open: Some(1.1),
            tilt: Some(-0.3),
            scale: None,
        };
        f.eyes[1].open = Some(0.8);
    }
    if inseg(q, 0.75, 0.9) {
        f.off.0 += (seg(q, 0.75, 0.9) * PI * 3.0).sin() * 6.0;
    }
}

fn foggy(f: &mut Frame) {
    let (q, t) = (f.q, f.t);
    f.post.flicker = 1.0 - 0.5 * seg(q, 0.0, 0.2).min(1.0 - seg(q, 0.85, 1.0));
    f.post.overlay = Overlay::FogBand(t);
    f.e.open = 0.4;
    f.e.lower = 0.2;
    f.gaze = ((t * 2.0).sin() * 0.8, -0.2);
}

fn meh_clouds(f: &mut Frame) {
    let q = f.q;
    f.at(&CLOUD, -8.0 + q * 26.0, 0.0, Some(T_WIND), 1.0);
    f.set(&Expr::MEH, seg(q, 0.0, 0.2));
    f.gaze = (lerp(-1.0, 1.0, q), -0.9);
    if inseg(q, 0.7, 0.9) {
        f.off.1 -= 5.0 * bump(q, 0.7, 0.9);
    }
}

fn heatwave(f: &mut Frame) {
    let t = f.t;
    f.e.open = 0.55;
    f.e.lift = 3.0;
    f.off.1 += (t * 12.0).sin().abs() * 3.0;
    sweat(f, 19.0, 0.8);
    for k in 0..3 {
        let kk = k as f32;
        let fr = (t * 0.7 + kk / 3.0).fract();
        f.dot(
            8.0 + kk * 4.0 + (fr * 12.0).sin() * 0.7,
            3.0 - fr * 4.0,
            Some(T_PROM),
            (1.0 - fr) * 0.7,
        );
    }
}

fn freezing(f: &mut Frame) {
    let (q, t) = (f.q, f.t);
    let s = seg(q, 0.0, 0.2) * (1.0 - seg(q, 0.85, 1.0));
    f.off.0 += (t * 60.0).sin() * 2.2 * s;
    f.e.open = 1.0 - 0.85 * s;
    f.e.scale = 1.0 - 0.15 * s;
    f.e.sep = 1.0 - 0.15 * s;
    f.e.lower = 0.3 * s;
}

// ---------------- sky and air events ----------------

fn lightning(f: &mut Frame) {
    let (q, t) = (f.q, f.t);
    if inseg(q, 0.02, 0.08) || inseg(q, 0.14, 0.19) {
        f.post.flicker = 3.0;
    }
    if inseg(q, 0.02, 0.35) {
        f.off.1 -= 10.0 * bump(q, 0.02, 0.35);
        f.e.open = 1.4;
    }
    if inseg(q, 0.35, 0.85) {
        f.e.open = 0.05;
        f.off.0 += (t * 60.0).sin() * 1.5;
    }
    f.hold(&Expr::SCARED, 0.85, 1.0);
}

fn uh_oh_rain(f: &mut Frame) {
    let (q, t) = (f.q, f.t);
    for k in 0..4 {
        let kk = k as f32;
        let fr = (t + kk * 0.25).fract();
        f.dot(
            4.0 + kk * 5.0,
            -1.0 + fr * 12.0,
            Some(T_WX),
            0.9 * (1.0 - fr * 0.3) * seg(q, 0.2, 0.4),
        );
    }
    f.gaze = (0.2, lerp(0.9, -1.0, seg(q, 0.1, 0.3)));
    if inseg(q, 0.3, 0.9) {
        f.e.open = 0.5;
    }
}

fn cough(f: &mut Frame) {
    let q = f.q;
    for (a, b) in [(0.1, 0.35), (0.45, 0.7)] {
        if inseg(q, a, b) {
            let s = seg(q, a, b);
            f.off.1 += 8.0 * (s * PI).sin();
            f.e.open = 0.08;
            for k in 0..3 {
                let kk = k as f32;
                f.dot(
                    11.0 + kk + s * 3.0,
                    15.0 + kk * 0.5 + s * 2.0,
                    None,
                    1.0 - s,
                );
            }
        }
    }
    f.hold(&Expr::WORRIED, 0.8, 1.0);
}

fn morning(f: &mut Frame) {
    let q = f.q;
    f.icon_alpha = 0.0;
    let s = ease(seg(q, 0.0, 0.6));
    let y = f.slot_y - s * 8.0;
    f.at(&SUN, SLOT_X, y, Some(T_SKY), 1.0);
    f.gaze = (0.0, 0.9 - s * 0.6);
    if inseg(q, 0.2, 0.7) {
        f.e.open = 0.3;
        f.e.lower = 0.4;
    }
    f.hold(&Expr::HAPPY, 0.7, 0.9);
}

fn evening(f: &mut Frame) {
    let q = f.q;
    if inseg(q, 0.3, 0.65) {
        let r = bump(q, 0.3, 0.65);
        f.e.open = 1.0 - 0.95 * r;
        f.rot = -0.1 * r;
        f.off.1 -= 4.0 * r;
    }
    if q >= 0.65 {
        f.e.open = 0.5;
        f.e.lower = 0.25;
    }
}

fn awoo(f: &mut Frame) {
    let (q, t) = (f.q, f.t);
    if inseg(q, 0.15, 0.85) {
        f.off.1 -= 6.0;
        f.rot = -0.15;
        f.e.open = 0.08;
        for i in 0..3 {
            let ii = i as f32;
            let r = (t * 0.6 + ii / 3.0).fract();
            f.at(
                &NOTE,
                9.0 + r * 3.0 + ii,
                8.0 - r * 8.0 - ii,
                Some(T_SKY),
                1.0 - r,
            );
        }
    }
}

fn look_up(f: &mut Frame) {
    let q = f.q;
    let s = seg(q, 0.1, 0.9);
    let gx = -1.0 + s * 25.0;
    let gy = 3.0 - (s * PI).sin() * 3.0;
    f.dot(gx, gy, Some(T_ISS), 1.0);
    f.gaze = ((gx - 12.0) / 10.0, -1.0);
    if inseg(q, 0.15, 0.85) {
        f.e.open = 1.2;
    }
}

// ---------------- prices ----------------

fn ka_ching(f: &mut Frame) {
    let q = f.q;
    if inseg(q, 0.05, 0.75) {
        f.sprite = Some(both(&EUROEYE));
    }
    let s = seg(q, 0.05, 0.6);
    f.at(
        &COIN,
        -2.0 + s * 8.0,
        19.0 - (s * PI * 3.0).sin().abs() * (5.0 - s * 4.0),
        Some(T_PRICE),
        1.0,
    );
    f.hold(&Expr::HAPPY, 0.75, 1.0);
}

fn expensive(f: &mut Frame) {
    let q = f.q;
    let s = ease(seg(q, 0.1, 0.7));
    if s < 1.0 {
        f.at(&COIN, SLOT_X - 1.0 - s * 10.0, 19.0, Some(T_PRICE), 1.0);
    }
    f.gaze = (lerp(0.0, -1.4, s), 0.9);
    f.eyes[0].open = Some(0.5);
    f.eyes[1].open = Some(1.1);
}

// ---------------- boot and idle habits ----------------

fn wake_up(f: &mut Frame) {
    let (q, t) = (f.q, f.t);
    f.set(&Expr::SLEEPY, 1.0);
    if q < 0.35 {
        let fade = 1.0 - seg(q, 0.25, 0.35);
        for i in 0..3 {
            let ii = i as f32;
            let r = (t * 0.5 + ii / 3.0).fract();
            f.at(
                &Z,
                17.0 + r * 3.0 + ii * 1.5,
                7.0 - r * 4.0 - ii,
                None,
                (1.0 - r) * fade,
            );
        }
    }
    if inseg(q, 0.35, 0.7) {
        let r = bump(q, 0.35, 0.7);
        f.e.open = 0.06;
        f.e.sep = 1.0 + 0.15 * r;
        f.e.scale = 1.0 + 0.1 * r;
        f.rot = -0.1 * r;
        f.off.1 -= r * 4.0;
    }
    if q >= 0.7 {
        let r = seg(q, 0.7, 1.0);
        f.set(&Expr::CONTENT, r);
        if (r * PI * 4.0).sin() > 0.8 {
            f.e.open = 0.05;
        }
    }
}

fn sneeze(f: &mut Frame) {
    let q = f.q;
    if inseg(q, 0.05, 0.5) {
        let s = seg(q, 0.05, 0.5);
        f.e.open = 0.6 - s * 0.5;
        f.e.tilt = -0.4 * s;
        f.off.1 -= s * 8.0;
        f.rot = -s * 0.12;
    }
    if inseg(q, 0.5, 0.8) {
        let s = seg(q, 0.5, 0.8);
        f.off.1 += 10.0 * (1.0 - s);
        f.rot = 0.15 * (1.0 - s);
        f.e.open = 0.06;
        for k in 0..4 {
            let kk = k as f32;
            f.dot(
                9.0 + kk * 1.5 + s * 5.0,
                17.0 + kk * 0.5 + s * 3.0,
                None,
                1.0 - s,
            );
        }
    }
}

fn humming(f: &mut Frame) {
    let (q, t) = (f.q, f.t);
    f.e.open = 0.1;
    f.e.lower = 0.5;
    f.off.0 += (t * 3.5).sin() * 3.0;
    f.rot = (t * 3.5).sin() * 0.06;
    let a = seg(q, 0.0, 0.15) * (1.0 - seg(q, 0.85, 1.0));
    f.at(&NOTE, 17.0, 3.0 + (t * 7.0).sin() * 0.6, Some(T_SKY), a);
}

fn peek(f: &mut Frame) {
    let q = f.q;
    let s = ease(seg(q, 0.05, 0.4)) * (1.0 - ease(seg(q, 0.7, 0.95)));
    f.off.0 += 26.0 * s;
    f.gaze = (1.4 * s, 0.0);
    f.eyes[0].scale = Some(1.0 - 0.3 * s);
    f.eyes[1].scale = Some(1.0 - 0.3 * s);
    f.e.open = 1.0 - 0.2 * s;
}

fn stretch(f: &mut Frame) {
    let q = f.q;
    let s = (ease(seg(q, 0.05, 0.85)) * PI).sin();
    f.e.open = 1.0 - 0.92 * s;
    f.e.scale = 1.0 + 0.2 * s;
    f.e.sep = 1.0 + 0.15 * s;
    f.off.1 -= 5.0 * s;
}

fn scanning(f: &mut Frame) {
    let q = f.q;
    f.e.open = 0.3;
    let s = (seg(q, 0.05, 0.95) * PI * 2.0).sin();
    f.gaze = (s * 1.3, 0.0);
    let a = PI + s * 1.2;
    f.dot(
        11.5 + (a + PI / 2.0).cos() * 10.5,
        11.5 + (a + PI / 2.0).sin() * 10.5,
        Some(T_K8S),
        1.0,
    );
}

fn hic(f: &mut Frame) {
    let q = f.q;
    if inseg(q, 0.2, 0.5) {
        let s = bump(q, 0.2, 0.5);
        f.off.1 -= 12.0 * s;
        f.e.open = 1.35;
        f.e.scale = 0.92;
    }
}

fn dozing_off(f: &mut Frame) {
    let q = f.q;
    let s = seg(q, 0.0, 0.7);
    f.e.open = 1.0 - s * 0.85;
    f.off.1 += s * s * 12.0;
    f.rot = s * 0.1;
    if inseg(q, 0.7, 0.8) {
        f.e.open = 1.3;
        f.off.1 -= s * s * 12.0;
    }
    if q >= 0.8 {
        f.e.open = 0.7;
    }
}

fn dizzy(f: &mut Frame) {
    let t = f.t;
    let odd = ((t * 6.0).floor() as i64) % 2 == 1;
    f.sprite = Some(both(if odd { &SPIRAL1 } else { &SPIRAL2 }));
    f.off.0 += (t * 3.0).cos() * 5.0;
    f.off.1 += (t * 3.0).sin() * 4.0;
    f.rot = (t * 3.0).sin() * 0.1;
    f.hold(&Expr::WORRIED, 0.85, 1.0);
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

    #[test]
    fn task_six_acts_exist_with_their_sources() {
        let d = ActKind::Launch.def();
        assert!(
            std::ptr::eq(d.icon.unwrap(), &ROCKET)
                && d.tint == Some(T_ARGO)
                && d.mood == Mood::Happy
        );
        let d = ActKind::SoFull.def();
        assert!(std::ptr::eq(d.icon.unwrap(), &DISK));
        let f = run_act(&d, 0.6, Expr::CONTENT, (0.0, 0.0));
        assert!(
            f.placed[0].sprite.rows[6] == "#######",
            "disk fills from the bottom"
        );
        assert!(ActKind::LightsFlicker.is_severe() && ActKind::Hello.is_severe());
        let f = run_act(
            &ActKind::LightsFlicker.def(),
            0.15,
            Expr::CONTENT,
            (0.0, 0.0),
        );
        assert!(
            f.post.flicker < 0.5 || f.post.flicker > 0.9,
            "flicker toggles"
        );
        let f = run_act(&ActKind::Alarm.def(), 0.5, Expr::CONTENT, (0.0, 0.0));
        assert!(f.post.rim > 0.4);
        let f = run_act(&ActKind::GotIt.def(), 0.5, Expr::CONTENT, (0.0, 0.0));
        assert!(f
            .sprite
            .is_some_and(|s| std::ptr::eq(s[0], &SQZ_L) && std::ptr::eq(s[1], &SQZ_R)));
        let f = run_act(&ActKind::Hmm.def(), 0.5, Expr::CONTENT, (0.0, 0.0));
        assert!(
            f.eyes[0].open.unwrap() < f.eyes[1].open.unwrap(),
            "suspicious: left narrow, right wide"
        );
        assert!(f.rot < 0.0);
    }

    #[test]
    fn weather_category_priority_and_codes() {
        use WeatherCat::*;
        assert_eq!(weather_cat(0, 20.0, 10.0, true), Some(Sunny));
        assert_eq!(weather_cat(1, 20.0, 10.0, true), Some(Sunny));
        assert_eq!(
            weather_cat(0, 20.0, 10.0, false),
            None,
            "clear night: nothing"
        );
        assert_eq!(weather_cat(2, 20.0, 10.0, true), Some(Clouds));
        assert_eq!(weather_cat(3, 20.0, 10.0, false), Some(Clouds));
        assert_eq!(weather_cat(45, 20.0, 10.0, true), Some(Fog));
        assert_eq!(weather_cat(48, 20.0, 10.0, true), Some(Fog));
        assert_eq!(weather_cat(61, 20.0, 10.0, true), Some(Rain));
        assert_eq!(weather_cat(80, 20.0, 10.0, true), Some(Rain));
        assert_eq!(weather_cat(71, -3.0, 10.0, true), Some(Snow));
        assert_eq!(
            weather_cat(85, -3.0, 60.0, true),
            Some(Snow),
            "snow beats wind and cold"
        );
        assert_eq!(
            weather_cat(61, 5.0, 60.0, true),
            Some(Rain),
            "rain beats wind"
        );
        assert_eq!(
            weather_cat(0, 30.0, 60.0, true),
            Some(Wind),
            "wind beats heat"
        );
        assert_eq!(weather_cat(0, 30.0, 10.0, true), Some(Hot));
        assert_eq!(weather_cat(0, 28.0, 10.0, true), Some(Hot));
        assert_eq!(weather_cat(0, 0.0, 10.0, true), Some(Cold));
        assert_eq!(
            weather_cat(2, -1.0, 10.0, true),
            Some(Cold),
            "cold beats clouds"
        );
        assert_eq!(
            weather_cat(45, -1.0, 10.0, true),
            Some(Fog),
            "fog beats cold"
        );
        assert_eq!(
            weather_cat(95, 20.0, 10.0, true),
            None,
            "thunder is an event"
        );
        assert_eq!(Snow.act(), ActKind::Snowing);
        assert_eq!(Sunny.act(), ActKind::Sunny);
        for c in [Snow, Rain, Wind, Fog, Cold, Hot, Clouds, Sunny] {
            assert!(c.act().is_habit(), "{c:?}");
        }
    }

    #[test]
    fn task_seven_acts_exist() {
        assert!(ActKind::Lightning.is_severe());
        assert!(!ActKind::Lightning.is_habit());
        let f = run_act(&ActKind::Sunny.def(), 0.6, Expr::CONTENT, (0.0, 0.0));
        assert!(f.sprite.is_some_and(|s| std::ptr::eq(s[0], &SHADE)));
        let f = run_act(&ActKind::Foggy.def(), 0.5, Expr::CONTENT, (0.0, 0.0));
        assert!((f.post.flicker - 0.5).abs() < 1e-3);
        assert!(matches!(f.post.overlay, Overlay::FogBand(_)));
        let f = run_act(&ActKind::Lightning.def(), 0.13, Expr::CONTENT, (0.0, 0.0));
        assert!(
            f.post.flicker > 1.5 || f.post.flicker < 1.2,
            "flash or not, never nan"
        );
        let f = run_act(&ActKind::KaChing.def(), 0.5, Expr::CONTENT, (0.0, 0.0));
        assert!(f.sprite.is_some_and(|s| std::ptr::eq(s[0], &EUROEYE)));
        let f = run_act(&ActKind::Morning.def(), 0.5, Expr::CONTENT, (0.0, 0.0));
        assert!(
            f.placed
                .iter()
                .any(|p| std::ptr::eq(p.sprite, &SUN) && p.y < SLOT_Y - 2.0),
            "sun rises out of the slot"
        );
    }

    #[test]
    fn task_eight_acts_complete_the_catalogue() {
        assert_eq!(ActKind::ALL.len(), 56);
        assert!(!ActKind::WakeUp.is_habit() && !ActKind::Dizzy.is_habit());
        assert!(ActKind::Sneeze.is_habit() && ActKind::DozingOff.is_habit());
        assert_eq!(IDLE_HABITS.len(), 7);
        assert_eq!(IDLE_HABITS.iter().map(|(_, w)| w).sum::<u32>(), 18);
        for (k, _) in IDLE_HABITS {
            assert!(
                k.is_idle_habit() && k.def().icon.is_none() && !k.def().rise,
                "{k:?}"
            );
        }
        assert!(!ActKind::Sunny.is_idle_habit());
        let d = ActKind::WakeUp.def();
        assert!(d.icon.is_none() && !d.rise && (d.dur - 3.6).abs() < 1e-6);
        let f = run_act(&d, 0.2, Expr::CONTENT, (0.0, 0.0));
        assert!(f.e.open < 0.4, "asleep at the start: {}", f.e.open);
        assert!(f.placed.iter().any(|p| std::ptr::eq(p.sprite, &Z)));
        let f = run_act(&ActKind::Dizzy.def(), 0.5, Expr::CONTENT, (0.0, 0.0));
        assert!(f
            .sprite
            .is_some_and(|s| std::ptr::eq(s[0], &SPIRAL1) || std::ptr::eq(s[0], &SPIRAL2)));
        let f = run_act(&ActKind::Peek.def(), 0.45, Expr::CONTENT, (0.0, 0.0));
        assert!(f.off.0 > 15.0, "leans to the edge: {}", f.off.0);
    }
}
