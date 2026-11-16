use core::sync::atomic::{AtomicBool, Ordering};
use obs_scene::item_scale_filter::set;

#[test]
fn test_scene_rotated_scale_filter_setter_state() {
    // The C render test selects these filters before checking its output.
    // Render statistics remain covered by the unchanged Tier 2 test.
    for initial in [false, true] {
        for filter in [0, 1, 2, 3, 4, 5, 6, u32::MAX, 0x8000_0000] {
            let mut stored = filter;
            let dirty = AtomicBool::new(initial);
            set(Some((&mut stored, &dirty)), filter, || panic!("live item"));
            assert_eq!(stored, filter);
            assert!(dirty.load(Ordering::SeqCst));
            dirty.store(false, Ordering::SeqCst);
            set(Some((&mut stored, &dirty)), !filter, || panic!("live item"));
            assert_eq!(stored, !filter);
            assert!(dirty.load(Ordering::SeqCst));
        }
    }
}

#[test]
fn missing_item_reports_once() {
    let mut calls = 0;
    set(None, u32::MAX, || calls += 1);
    assert_eq!(calls, 1);
}
