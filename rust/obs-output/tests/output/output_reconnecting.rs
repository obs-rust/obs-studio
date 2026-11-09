//! Mirrors test_encoder_output_getters_characterization.c and reconnect lifecycle (#4104).
use obs_output::output_reconnecting::get;

#[test]
fn mirrors_initial_false() {
    assert!(!get(Some(false), |_| panic!("unexpected log")));
}

#[test]
fn mirrors_null_false() {
    let mut logs = Vec::new();
    assert!(!get(None, |name| logs.push(name)));
    assert_eq!(logs, [c"obs_output_reconnecting"]);
}

#[test]
fn mirrors_reconnecting_true() {
    assert!(get(Some(true), |_| panic!("unexpected log")));
}
