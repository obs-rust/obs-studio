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

/// Mirrors the ID store observed by scene_item_save_and_add.
#[test]
fn scene_item_save_and_add_set_id() {
    let mut stored = 0;
    obs_scene::item_id::set_id(&mut stored, 4242);
    assert_eq!(get_id(Some(stored), || panic!("live item")), 4242);
}

#[test]
fn set_id_signed_edges_and_repeated_stores() {
    let mut stored = 999;
    for id in [i64::MIN, i64::MIN + 1, -17, -1, 0, 1, 4242, i64::MAX - 1, i64::MAX] {
        obs_scene::item_id::set_id(&mut stored, id);
        assert_eq!(stored, id);
        obs_scene::item_id::set_id(&mut stored, id);
        assert_eq!(stored, id);
    }
}
