use obs_graphics::linear_srgb::linear_srgb;

/// Mirrors test_graphics_linear_srgb.c's absent, false, true and repeated reads.
#[test]
fn test_graphics_linear_srgb() {
    assert!(!linear_srgb(None));
    assert!(!linear_srgb(Some(false)));
    assert!(linear_srgb(Some(true)));
    assert!(linear_srgb(Some(true)));
    assert!(!linear_srgb(Some(false)));
    assert!(!linear_srgb(None));
}

/// Mirrors test_graphics_set_linear_srgb.c, including repeated writes and reads.
#[test]
fn test_graphics_set_linear_srgb() {
    use obs_graphics::linear_srgb::set_linear_srgb;
    assert!(!set_linear_srgb(None, true));
    for previous in [false, true] {
        for next in [false, true] {
            let mut value = previous;
            assert_eq!(set_linear_srgb(Some(&mut value), next), previous);
            assert_eq!(value, next);
            assert_eq!(set_linear_srgb(Some(&mut value), next), next);
            assert_eq!(linear_srgb(Some(value)), next);
        }
    }
    assert!(!set_linear_srgb(None, false));
}
