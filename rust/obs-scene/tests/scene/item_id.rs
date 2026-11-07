//! Tier 1 mirrors test_scene_item_id.c through the safe core.
use obs_scene::item_id::get_id;

#[test]
fn null_item() {
    let mut logs = 0;
    assert_eq!(get_id(None, || logs += 1), 0);
    assert_eq!(logs, 1);
}

#[test]
fn stored_ids() {
    for id in [
        i64::MIN,
        i64::MIN + 1,
        -17,
        -1,
        0,
        1,
        17,
        i64::MAX - 1,
        i64::MAX,
    ] {
        let before = id;
        assert_eq!(get_id(Some(id), || panic!("live item must not log")), id);
        assert_eq!(id, before);
    }
}
