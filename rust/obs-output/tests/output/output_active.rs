//! Mirrors test_encoder_output_getters_characterization.c and lifecycle cases (#4111).
use obs_output::output_active::get;

#[test]
fn mirrors_initial_false() {
    assert!(!get(Some(false), || false));
}

#[test]
fn mirrors_null_false_without_reading_flags() {
    assert!(!get(None, || panic!("NULL must not read reconnecting")));
}

#[test]
fn active_short_circuits_reconnecting() {
    assert!(get(Some(true), || panic!("active must short circuit")));
}

#[test]
fn reconnecting_keeps_output_active() {
    assert!(get(Some(false), || true));
}
