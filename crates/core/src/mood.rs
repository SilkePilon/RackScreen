//! The face's mood: the first matching rack state wins.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mood {
    Scared,
    Sad,
    Angry,
    Hot,
    Worried,
    Sleepy,
    Happy,
    Content,
}

impl Mood {
    pub const ALL: [Mood; 8] = [
        Mood::Scared,
        Mood::Sad,
        Mood::Angry,
        Mood::Hot,
        Mood::Worried,
        Mood::Sleepy,
        Mood::Happy,
        Mood::Content,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Mood::Scared => "scared",
            Mood::Sad => "sad",
            Mood::Angry => "angry",
            Mood::Hot => "hot",
            Mood::Worried => "worried",
            Mood::Sleepy => "sleepy",
            Mood::Happy => "happy",
            Mood::Content => "content",
        }
    }
}

/// The rack facts the mood is read from, already reduced to booleans. The CI
/// and restart inputs come from later sources; without them they stay false.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MoodInputs {
    pub on_battery: bool,
    pub nodes_not_ready: bool,
    pub alerts: bool,
    pub app_degraded: bool,
    pub volume_degraded: bool,
    pub hot: bool,
    pub pods_failed: bool,
    pub recent_restart: bool,
    pub bedtime: bool,
    pub ci_passed_recently: bool,
    pub cheap_price: bool,
    pub clear_day: bool,
}

/// First match wins, in the spec's order.
pub fn mood(i: &MoodInputs) -> Mood {
    if i.on_battery {
        Mood::Scared
    } else if i.nodes_not_ready {
        Mood::Sad
    } else if i.alerts || i.app_degraded || i.volume_degraded {
        Mood::Angry
    } else if i.hot {
        Mood::Hot
    } else if i.pods_failed || i.recent_restart {
        Mood::Worried
    } else if i.bedtime {
        Mood::Sleepy
    } else if i.ci_passed_recently || i.cheap_price || i.clear_day {
        Mood::Happy
    } else {
        Mood::Content
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn priority_table_first_match_wins() {
        let all = MoodInputs {
            on_battery: true,
            nodes_not_ready: true,
            alerts: true,
            app_degraded: true,
            volume_degraded: true,
            hot: true,
            pods_failed: true,
            recent_restart: true,
            bedtime: true,
            ci_passed_recently: true,
            cheap_price: true,
            clear_day: true,
        };
        let mut i = all;
        assert_eq!(mood(&i), Mood::Scared);
        i.on_battery = false;
        assert_eq!(mood(&i), Mood::Sad);
        i.nodes_not_ready = false;
        assert_eq!(mood(&i), Mood::Angry);
        i.alerts = false;
        assert_eq!(mood(&i), Mood::Angry, "a degraded app is enough");
        i.app_degraded = false;
        assert_eq!(mood(&i), Mood::Angry, "so is a degraded volume");
        i.volume_degraded = false;
        assert_eq!(mood(&i), Mood::Hot);
        i.hot = false;
        assert_eq!(mood(&i), Mood::Worried);
        i.pods_failed = false;
        assert_eq!(mood(&i), Mood::Worried, "a recent restart worries too");
        i.recent_restart = false;
        assert_eq!(mood(&i), Mood::Sleepy);
        i.bedtime = false;
        assert_eq!(mood(&i), Mood::Happy);
        i.ci_passed_recently = false;
        i.cheap_price = false;
        assert_eq!(mood(&i), Mood::Happy, "a clear day alone is happy");
        i.clear_day = false;
        assert_eq!(mood(&i), Mood::Content);
        assert_eq!(mood(&MoodInputs::default()), Mood::Content);
        assert_eq!(Mood::ALL.len(), 8);
        assert_eq!(Mood::Sleepy.name(), "sleepy");
    }
}
