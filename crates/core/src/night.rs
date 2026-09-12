//! Quiet-hours window math. Time zone handling happens in the binary.

pub fn parse_hhmm(s: &str) -> Option<u32> {
    let (h, m) = s.trim().split_once(':')?;
    let h: u32 = h.parse().ok()?;
    let m: u32 = m.parse().ok()?;
    (h < 24 && m < 60).then_some(h * 60 + m)
}

/// True when `now_min` lies inside [start, end). Windows may wrap midnight.
/// start == end disables the window.
pub fn is_night(now_min: u32, start_min: u32, end_min: u32) -> bool {
    if start_min == end_min {
        return false;
    }
    if start_min < end_min {
        (start_min..end_min).contains(&now_min)
    } else {
        now_min >= start_min || now_min < end_min
    }
}

/// Minutes before `start` the face turns sleepy.
pub const BEDTIME_BEFORE_MIN: u32 = 30;
/// Minutes after `end` the face is still waking up.
pub const BEDTIME_AFTER_MIN: u32 = 10;

/// True in the half hour before the night window opens and the ten minutes
/// after it closes: the displays are off during the night itself.
pub fn bedtime_near(now_min: u32, start_min: u32, end_min: u32) -> bool {
    if start_min == end_min {
        return false;
    }
    let day = 24 * 60;
    let before_start = (start_min + day - BEDTIME_BEFORE_MIN) % day;
    let after_end = (end_min + BEDTIME_AFTER_MIN) % day;
    is_night(now_min, before_start, start_min) || is_night(now_min, end_min, after_end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_times() {
        assert_eq!(parse_hhmm("23:00"), Some(1380));
        assert_eq!(parse_hhmm("07:05"), Some(425));
        assert_eq!(parse_hhmm("24:00"), None);
        assert_eq!(parse_hhmm("x"), None);
    }

    #[test]
    fn wrapping_window() {
        let (s, e) = (1380, 420);
        assert!(is_night(1390, s, e));
        assert!(is_night(0, s, e));
        assert!(is_night(419, s, e));
        assert!(!is_night(420, s, e));
        assert!(!is_night(720, s, e));
    }

    #[test]
    fn plain_window_and_disabled() {
        assert!(is_night(120, 60, 360));
        assert!(!is_night(360, 60, 360));
        assert!(!is_night(120, 300, 300));
    }

    #[test]
    fn bedtime_is_half_an_hour_before_and_ten_minutes_after() {
        let (s, e) = (1380, 420); // 23:00 .. 07:00
        assert!(!bedtime_near(1349, s, e)); // 22:29
        assert!(bedtime_near(1350, s, e)); // 22:30
        assert!(bedtime_near(1379, s, e)); // 22:59
        assert!(!bedtime_near(1380, s, e)); // 23:00 is night, displays off
        assert!(!bedtime_near(0, s, e));
        assert!(bedtime_near(420, s, e)); // 07:00
        assert!(bedtime_near(429, s, e)); // 07:09
        assert!(!bedtime_near(430, s, e)); // 07:10
        assert!(!bedtime_near(720, s, e));
    }

    #[test]
    fn bedtime_wraps_midnight_and_ignores_a_disabled_window() {
        // 00:10 .. 06:00: the half hour before starts at 23:40
        assert!(bedtime_near(1420, 10, 360));
        assert!(bedtime_near(5, 10, 360));
        assert!(!bedtime_near(10, 10, 360));
        assert!(!bedtime_near(100, 300, 300));
    }
}
