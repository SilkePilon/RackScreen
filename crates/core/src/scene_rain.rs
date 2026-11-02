//! Buienradar nowcast helpers: two hours in five-minute slots.

pub const RAIN_SLOTS: usize = 24;
pub const SLOT_SECS: i64 = 300;
/// A slot at or above this counts as rain.
pub const WET_MM: f32 = 0.1;
/// Rain within this many minutes counts as "soon".
pub const SOON_MINUTES: u32 = 15;

/// The slots still ahead: elapsed ones are dropped from the front so slot 0
/// is always the current five minutes.
pub fn current_slots(from: i64, mm_per_h: &[f32], unix_now: i64) -> Vec<f32> {
    let elapsed = ((unix_now - from).max(0) / SLOT_SECS) as usize;
    mm_per_h.iter().copied().skip(elapsed).collect()
}

/// Minutes until the first wet slot, `None` when the window is dry.
pub fn first_wet_minutes(slots: &[f32]) -> Option<u32> {
    slots
        .iter()
        .position(|v| *v >= WET_MM)
        .map(|i| (i as i64 * SLOT_SECS / 60) as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slots(wet: &[(usize, f32)]) -> Vec<f32> {
        let mut v = vec![0.0f32; 24];
        for (i, mm) in wet {
            v[*i] = *mm;
        }
        v
    }
    #[test]
    fn elapsed_slots_drop_off_the_front() {
        let s = slots(&[(3, 2.0)]);
        assert_eq!(current_slots(1000, &s, 1000).len(), 24);
        let later = current_slots(1000, &s, 1000 + 2 * SLOT_SECS + 10);
        assert_eq!(later.len(), 22);
        assert_eq!(later[1], 2.0);
        assert!(current_slots(1000, &s, 1000 + 30 * SLOT_SECS).is_empty());
        assert_eq!(
            current_slots(1000, &s, 900).len(),
            24,
            "clock behind: nothing dropped"
        );
    }
    #[test]
    fn first_wet_minutes_finds_the_first_wet_slot() {
        assert_eq!(first_wet_minutes(&slots(&[(5, 0.3)])), Some(25));
        assert_eq!(first_wet_minutes(&slots(&[(0, 1.0)])), Some(0));
        assert_eq!(first_wet_minutes(&slots(&[])), None);
        assert_eq!(
            first_wet_minutes(&slots(&[(2, 0.05)])),
            None,
            "below the wet threshold"
        );
    }
}
