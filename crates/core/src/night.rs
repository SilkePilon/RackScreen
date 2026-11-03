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
}
