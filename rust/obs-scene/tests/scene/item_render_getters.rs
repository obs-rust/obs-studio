//! Tier 1 mirrors test_scene_item_render_getters.c.
use obs_scene::item_render_getters::get;

#[test]
fn null_items() {
    let mut logs = 0;
    for _ in 0..3 {
        assert_eq!(get(None, || logs += 1), 0);
    }
    assert_eq!(logs, 3);
}

#[test]
fn stored_values() {
    let values = [0, 1, 2, 3, 4, 5, 6, 7, 17, 0x7fff_ffff];
    for (i, scale) in values.into_iter().enumerate() {
        for (j, method) in values.into_iter().enumerate() {
            for value in [scale, method, values[(i + j) % 10]] {
                assert_eq!(get(Some(value), || panic!("non-null logged")), value);
            }
        }
    }
    for value in [0x8000_0000, u32::MAX] {
        assert_eq!(get(Some(value), || panic!("non-null logged")), value);
    }
}
