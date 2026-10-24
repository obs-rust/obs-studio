//! The countdown progress.

use obs_fallback_warning::{COUNTDOWN_MS, progress_percent};

#[test]
fn the_countdown_is_3_seconds() {
    assert_eq!(COUNTDOWN_MS, 3_000);
}

#[test]
fn progress_goes_from_0_to_100() {
    assert_eq!(progress_percent(0), 0);
    assert_eq!(progress_percent(30), 1);
    assert_eq!(progress_percent(1_500), 50);
    assert_eq!(progress_percent(2_999), 99);
    assert_eq!(progress_percent(3_000), 100);
}

#[test]
fn progress_stays_at_100_after_the_end() {
    assert_eq!(progress_percent(3_001), 100);
    assert_eq!(progress_percent(u64::MAX), 100);
}
