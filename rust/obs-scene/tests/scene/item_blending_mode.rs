use obs_scene::item_blending_mode::set;

#[test]
fn test_scaled_mix_alpha_and_test_darken_transparent_modes() {
    // The existing rendering tests select NORMAL (0), SUBTRACT (2), and DARKEN (6).
    for value in [0, 1, 2, 5, 6, 7, u32::MAX, 0x8000_0000] {
        let mut mode = 17;
        let mut dirty = false;
        set(Some(&mut mode), value, || dirty = true, || panic!("live item"));
        assert_eq!(mode, value);
        assert!(dirty);
        // An unchanged mode must still mark a fresh transform dirty.
        dirty = false;
        set(Some(&mut mode), value, || dirty = true, || panic!("live item"));
        assert!(dirty);
    }
}

#[test]
fn null_logs_without_touching_fields() {
    let mut logs = 0;
    set(None, u32::MAX, || panic!("null item"), || logs += 1);
    assert_eq!(logs, 1);
}
