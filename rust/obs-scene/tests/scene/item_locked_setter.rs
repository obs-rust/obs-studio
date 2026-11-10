//! Mirrors test_scene_item_locked_setter.c without FFI or unsafe code.
use obs_scene::item_locked_setter::set_locked;

#[test]
fn characterized_returns_state_and_signal_count() {
    for requested in [false, true] {
        assert_eq!(set_locked(None, false, requested), None);
        for parent in [false, true] {
            for initial in [false, true] {
                let update = set_locked(Some(initial), parent, requested);
                let changed = parent && initial != requested;
                assert_eq!(update.is_some(), changed);
                assert_eq!(
                    update.unwrap_or(initial),
                    if changed { requested } else { initial }
                );
                assert_eq!(update, changed.then_some(requested));
            }
        }
    }
}
