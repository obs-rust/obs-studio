//! Tier 1 mirrors `test/cmocka/test_display_state.c` without FFI.
use obs_core::display_state::{enabled, set_background_color, set_enabled};

#[test]
fn test_null() {
    assert!(!enabled(None));
    set_enabled(None, true);
    set_enabled(None, false);
    set_background_color(None, u32::MAX);
}

#[test]
fn test_enabled() {
    let mut flag = false;
    for value in [false, true, true, false, false, true] {
        set_enabled(Some(&mut flag), value);
        assert_eq!(enabled(Some(flag)), value);
    }
}

#[test]
fn test_color() {
    let mut color = 0;
    for value in [0, u32::MAX, 0x8000_0000, 0x0102_0304, 0xff00_0000, 0x00ff_ffff] {
        set_background_color(Some(&mut color), value);
        assert_eq!(color, value);
    }
}
