//! Tier 1 mirrors test_scene_item_state_getters.c through the safe core.
use obs_scene::item_state_getters::{locked, selected};

#[test]
fn null_item() {
    assert!(!selected(None));
    assert!(!locked(None));
}

#[test]
fn independent_flags() {
    for selected_flag in [false, true] {
        for locked_flag in [false, true] {
            let before = (selected_flag, locked_flag);
            assert_eq!(selected(Some(selected_flag)), selected_flag);
            assert_eq!(locked(Some(locked_flag)), locked_flag);
            assert_eq!((selected_flag, locked_flag), before);
        }
    }
}
