//! Mirrors test_scene_item_transition_getters.c without FFI or unsafe.
use obs_scene::item_transition_getters::{duration, transition};

#[test]
fn transition_getters_null_item() {
    for show in [false, true] {
        assert_eq!(transition::<usize>(None, show), 0);
        assert_eq!(duration(None, show), 0);
    }
}

#[test]
fn transition_getters_stored_fields_are_unchanged() {
    let values = [0, 1, 250, u32::MAX];
    for mask in 0..4 {
        for (i, &show_duration) in values.iter().enumerate() {
            let pointers = (if mask & 1 != 0 { 17 } else { 0 }, if mask & 2 != 0 { 29 } else { 0 });
            let durations = (show_duration, values[3 - i]);
            let before = (pointers, durations);
            for _ in 0..2 {
                assert_eq!(transition(Some(pointers), true), pointers.0);
                assert_eq!(transition(Some(pointers), false), pointers.1);
                assert_eq!(duration(Some(durations), true), durations.0);
                assert_eq!(duration(Some(durations), false), durations.1);
                assert_eq!((pointers, durations), before);
            }
        }
    }
}
