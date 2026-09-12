//! Which mood the face holds between acts: derived from cluster state, with a
//! short reaction hold after an act.

use crate::anim::Secs;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mood {
    Content,
    Happy,
    Excited,
    Worried,
    Sad,
    Angry,
    Hot,
    Scared,
    Bored,
    Sleepy,
}

impl Mood {
    /// Moods the cluster state forces regardless of any reaction.
    pub fn is_severe(self) -> bool {
        matches!(self, Mood::Scared | Mood::Sad | Mood::Angry | Mood::Hot)
    }
}

/// How long a crash keeps the face worried.
pub const CRASH_WORRY_SECS: Secs = 300.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MoodTuning {
    pub bored_after: Secs,
    pub reaction: Secs,
}

impl Default for MoodTuning {
    fn default() -> Self {
        Self {
            bored_after: 120.0 * 60.0,
            reaction: 60.0,
        }
    }
}

/// The cluster facts the state mood is derived from, already reduced to bools.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MoodInputs {
    pub on_battery: bool,
    pub nodes_not_ready: bool,
    pub app_degraded: bool,
    pub volume_degraded: bool,
    pub alerts: bool,
    pub hot: bool,
    pub pods_failed: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MoodEngine {
    reaction: Option<(Mood, Secs)>,
    last_event: Secs,
    crashed_at: Option<Secs>,
    bedtime_near: bool,
    tuning: MoodTuning,
}

impl MoodEngine {
    pub fn new(now: Secs) -> Self {
        Self {
            reaction: None,
            last_event: now,
            crashed_at: None,
            bedtime_near: false,
            tuning: MoodTuning::default(),
        }
    }
    pub fn set_tuning(&mut self, t: MoodTuning) {
        self.tuning = t;
    }
    pub fn tuning(&self) -> MoodTuning {
        self.tuning
    }
    /// Something happened: the bored timer restarts.
    pub fn touch(&mut self, now: Secs) {
        self.last_event = now;
    }
    pub fn crashed(&mut self, now: Secs) {
        self.crashed_at = Some(now);
        self.touch(now);
    }
    /// Hold `mood` for `tuning.reaction` seconds (unless the state is severe).
    /// `Mood::Content` is not a reaction: it clears any held one.
    pub fn react(&mut self, mood: Mood, now: Secs) {
        self.touch(now);
        self.reaction = if mood == Mood::Content {
            None
        } else {
            Some((mood, now))
        };
    }
    pub fn set_bedtime_near(&mut self, near: bool) {
        self.bedtime_near = near;
    }
    pub fn bedtime_near(&self) -> bool {
        self.bedtime_near
    }

    pub fn state_mood(&self, i: &MoodInputs, now: Secs) -> Mood {
        if i.on_battery {
            return Mood::Scared;
        }
        if i.nodes_not_ready {
            return Mood::Sad;
        }
        if i.app_degraded || i.volume_degraded || i.alerts {
            return Mood::Angry;
        }
        if i.hot {
            return Mood::Hot;
        }
        let recent_crash = self.crashed_at.is_some_and(|t| now - t < CRASH_WORRY_SECS);
        if i.pods_failed || recent_crash {
            return Mood::Worried;
        }
        if self.bedtime_near {
            return Mood::Sleepy;
        }
        if now - self.last_event > self.tuning.bored_after {
            return Mood::Bored;
        }
        Mood::Content
    }

    /// The mood the face holds right now.
    pub fn current(&self, i: &MoodInputs, now: Secs) -> Mood {
        let state = self.state_mood(i, now);
        if state.is_severe() {
            return state;
        }
        match self.reaction {
            Some((m, at)) if now - at <= self.tuning.reaction => m,
            _ => state,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quiet() -> MoodInputs {
        MoodInputs::default()
    }

    #[test]
    fn state_priority_scared_over_sad_over_angry_over_hot_over_worried() {
        let e = MoodEngine::new(0.0);
        let all = MoodInputs {
            on_battery: true,
            nodes_not_ready: true,
            app_degraded: true,
            volume_degraded: true,
            alerts: true,
            hot: true,
            pods_failed: true,
        };
        assert_eq!(e.state_mood(&all, 1.0), Mood::Scared);
        let mut i = all;
        i.on_battery = false;
        assert_eq!(e.state_mood(&i, 1.0), Mood::Sad);
        i.nodes_not_ready = false;
        assert_eq!(e.state_mood(&i, 1.0), Mood::Angry);
        i.app_degraded = false;
        i.volume_degraded = false;
        assert_eq!(e.state_mood(&i, 1.0), Mood::Angry, "alerts alone are angry");
        i.alerts = false;
        assert_eq!(e.state_mood(&i, 1.0), Mood::Hot);
        i.hot = false;
        assert_eq!(e.state_mood(&i, 1.0), Mood::Worried);
        i.pods_failed = false;
        assert_eq!(e.state_mood(&i, 1.0), Mood::Content);
    }

    #[test]
    fn a_crash_keeps_worried_for_five_minutes() {
        let mut e = MoodEngine::new(0.0);
        e.crashed(10.0);
        assert_eq!(e.state_mood(&quiet(), 100.0), Mood::Worried);
        assert_eq!(e.state_mood(&quiet(), 309.0), Mood::Worried);
        assert_eq!(e.state_mood(&quiet(), 311.0), Mood::Content);
    }

    #[test]
    fn reaction_holds_then_expires() {
        let mut e = MoodEngine::new(0.0);
        e.react(Mood::Excited, 100.0);
        assert_eq!(e.current(&quiet(), 100.0), Mood::Excited);
        assert_eq!(e.current(&quiet(), 159.0), Mood::Excited);
        assert_eq!(e.current(&quiet(), 161.0), Mood::Content);
    }

    #[test]
    fn severe_state_hides_a_reaction() {
        let mut e = MoodEngine::new(0.0);
        e.react(Mood::Excited, 100.0);
        let down = MoodInputs {
            nodes_not_ready: true,
            ..MoodInputs::default()
        };
        assert_eq!(e.current(&down, 101.0), Mood::Sad);
        // node back: happy reaction shows, then content
        e.react(Mood::Happy, 200.0);
        assert_eq!(e.current(&quiet(), 201.0), Mood::Happy);
        assert_eq!(e.current(&quiet(), 300.0), Mood::Content);
    }

    #[test]
    fn bored_after_quiet_and_not_after_boot() {
        let mut e = MoodEngine::new(0.0);
        e.set_tuning(MoodTuning {
            bored_after: 600.0,
            reaction: 60.0,
        });
        assert_eq!(e.current(&quiet(), 599.0), Mood::Content);
        assert_eq!(e.current(&quiet(), 601.0), Mood::Bored);
        e.touch(700.0);
        assert_eq!(e.current(&quiet(), 800.0), Mood::Content);
    }

    #[test]
    fn sleepy_beats_bored_and_loses_to_worried() {
        let mut e = MoodEngine::new(0.0);
        e.set_tuning(MoodTuning {
            bored_after: 10.0,
            reaction: 60.0,
        });
        e.set_bedtime_near(true);
        assert_eq!(e.current(&quiet(), 100.0), Mood::Sleepy);
        let failed = MoodInputs {
            pods_failed: true,
            ..MoodInputs::default()
        };
        assert_eq!(e.current(&failed, 100.0), Mood::Worried);
        e.set_bedtime_near(false);
        assert_eq!(e.current(&quiet(), 100.0), Mood::Bored);
    }

    #[test]
    fn reaction_beats_bored_and_sleepy() {
        let mut e = MoodEngine::new(0.0);
        e.set_bedtime_near(true);
        e.react(Mood::Worried, 5.0);
        assert_eq!(e.current(&quiet(), 6.0), Mood::Worried);
    }
}
