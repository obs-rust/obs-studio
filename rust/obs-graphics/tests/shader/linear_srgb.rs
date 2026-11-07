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
