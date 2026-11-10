//! Tier 1: source registry icon results, including registration size boundaries.
use obs_source::source_icon_type::icon_type;

#[test]
fn test_source_u32_field() {
    // test_module_register_copy_steps.c: end - 1 defaults the field to UNKNOWN;
    // end and next_end - 1 both preserve CAMERA.
    assert_eq!(icon_type(Some(0)), 0);
    assert_eq!(icon_type(Some(9)), 9);
    assert_eq!(icon_type(Some(9)), 9);
}

#[test]
fn all_registered_icons_and_missing_id() {
    assert_eq!(icon_type(None), 0);
    for icon in 0..=14 {
        assert_eq!(icon_type(Some(icon)), icon);
    }
    // Metadata is forwarded rather than validated or clamped.
    assert_eq!(icon_type(Some(u32::MAX)), u32::MAX);
}
