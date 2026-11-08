//! Mirrors test_encoder_roi_state_getters.c over the safe core.
use obs_output::encoder_roi_state::{has_roi, roi_increment};

#[test]
fn null_defaults_log_once() {
    let mut logs = 0;
    assert!(!has_roi(None, || logs += 1));
    assert_eq!(logs, 1);
    logs = 0;
    assert_eq!(roi_increment(None, || logs += 1), 0);
    assert_eq!(logs, 1);
}

#[test]
fn full_width_independent_state_reads() {
    for count in [0, 1, 2, usize::MAX] {
        for increment in [0, 1, 0x8000_0001, u32::MAX] {
            let mut logs = 0;
            assert_eq!(has_roi(Some(count), || logs += 1), count > 0);
            assert_eq!(roi_increment(Some(increment), || logs += 1), increment);
            assert!(has_roi(Some(1), || logs += 1));
            assert_eq!(roi_increment(Some(19), || logs += 1), 19);
            assert_eq!(logs, 0);
        }
    }
}
