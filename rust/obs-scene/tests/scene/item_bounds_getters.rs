//! Tier 1 mirrors the scalar assertions in test_scene_item_bounds_getters.c.
use obs_scene::item_bounds_getters::{bounds_alignment, bounds_crop, bounds_type};

#[test]
fn null_item() {
    assert_eq!(bounds_type(None), 0);
    assert_eq!(bounds_alignment(None), 0);
    assert!(!bounds_crop(None));
}

#[test]
fn stored_bounds_fields() {
    for kind in 0..=6 {
        for crop in [false, true] {
            let alignment = u32::MAX - kind;
            assert_eq!(bounds_type(Some(kind)), kind);
            assert_eq!(bounds_alignment(Some(alignment)), alignment);
            assert_eq!(bounds_crop(Some(crop)), crop);
        }
    }
}
