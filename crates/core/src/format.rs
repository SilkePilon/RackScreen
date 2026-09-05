//! Compact number formatting for the badge.

/// Bytes per second to a short badge string, e.g. `12.4M`, `850K`, `12B`.
pub fn fmt_speed(bps: i64) -> String {
    let b = bps.max(0) as f64;
    if b >= 1_048_576.0 {
        format!("{:.1}M", b / 1_048_576.0)
    } else if b >= 1024.0 {
        format!("{:.0}K", b / 1024.0)
    } else {
        format!("{}B", b as i64)
    }
}

/// Seconds to a short ETA, e.g. `45s`, `12m`, `1h05`. Unknown becomes `--`.
pub fn fmt_eta(secs: i64) -> String {
    if !(0..=864_000).contains(&secs) {
        return "--".into();
    }
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else {
        format!("{}h{:02}", secs / 3600, (secs % 3600) / 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speed_units() {
        assert_eq!(fmt_speed(13_002_342), "12.4M");
        assert_eq!(fmt_speed(870_400), "850K");
        assert_eq!(fmt_speed(12), "12B");
        assert_eq!(fmt_speed(-5), "0B");
    }

    #[test]
    fn eta_units() {
        assert_eq!(fmt_eta(45), "45s");
        assert_eq!(fmt_eta(720), "12m");
        assert_eq!(fmt_eta(3900), "1h05");
        assert_eq!(fmt_eta(-1), "--");
        assert_eq!(fmt_eta(8_640_000), "--");
    }
}
