//! Mirrors existing test_encoder_output_getters_characterization.c (#4100).
use obs_output::output_total_frames::get;

#[test]
fn mirrors_initial_zero() {
    assert_eq!(get(Some(0), |_| panic!("unexpected log")), 0);
}

#[test]
fn mirrors_null_zero() {
    let mut logs = Vec::new();
    assert_eq!(get(None, |name| logs.push(name)), 0);
    assert_eq!(logs, [c"obs_output_get_total_frames"]);
}

#[test]
fn narrows_c_long_to_int() {
    for (value, expected) in [(1, 1), (-1, -1), (i32::MAX as i64, i32::MAX),
        (i32::MAX as i64 + 1, i32::MIN), (i64::MAX, -1), (i64::MIN, 0)] {
        assert_eq!(get(Some(value), |_| panic!("unexpected log")), expected);
    }
}
