//! Tier 1 mirror of test_scene_item_parent_getter.c, using safe parent tokens.
use obs_scene::item_parent_getter::parent;

#[test]
fn parent_getter_characterization() {
    assert_eq!(parent::<usize>(None), 0);
    for bits in 0..16 {
        let stored = if bits & 4 != 0 { 1 + ((bits >> 3) & 1) } else { 0 };
        let before = (stored, bits & 1 != 0, bits & 2 != 0, -17);
        assert_eq!(parent(Some(stored)), stored);
        assert_eq!(parent(Some(stored)), stored);
        assert_eq!((stored, bits & 1 != 0, bits & 2 != 0, -17), before);
    }
}

#[test]
fn parent_token_is_preserved() {
    for stored in [0, 1, usize::MAX / 2, usize::MAX] {
        assert_eq!(parent(Some(stored)), stored);
    }
}
