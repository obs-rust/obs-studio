//! Tier 1 mirrors test_scene_item_visible.c without FFI or unsafe code.
use obs_scene::item_visible::visible;

#[test]
fn item_visible() {
    assert!(!visible(None));
    for bits in 0..32 {
        assert_eq!(visible(Some(bits & 8 != 0)), bits & 8 != 0);
    }
}
