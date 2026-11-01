//! `obs_util::cpu_usage` (#3367).

use obs_util::cpu_usage::percent;

#[test]
fn the_share_of_the_machine() {
    assert_eq!(percent(0.0, 100.0, 4.0), 0.0);
    assert_eq!(percent(100.0, 100.0, 4.0), 25.0);
    assert_eq!(percent(400.0, 100.0, 4.0), 100.0);
}

/// `times()` counts in 10 ms ticks. Over one tick of wall time the busy
/// threads can be charged one tick more than the cores could run (5 for 4
/// cores), which reads as 125 %: GetStats then reported more than 100 %
/// (test_ws_cpu_usage). No process uses more than all of the machine.
#[test]
fn a_coarse_sample_is_at_most_all_of_the_machine() {
    assert_eq!(percent(5.0, 1.0, 4.0), 100.0);
    assert_eq!(percent(16.0, 1.0, 8.0), 100.0);
}

/// The cap is C's `percent > 100.0 ? 100.0 : percent`: a NaN (an empty
/// interval, 0 / 0) passes through unchanged, as before.
#[test]
fn the_cap_keeps_a_nan() {
    assert!(obs_util::cpu_usage::cap(f64::NAN).is_nan());
    assert_eq!(obs_util::cpu_usage::cap(f64::INFINITY), 100.0);
}
