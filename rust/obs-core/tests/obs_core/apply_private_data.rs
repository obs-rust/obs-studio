//! Tier 1: one-to-one counterparts of the unchanged test_apply_private_data.c,
//! through the safe core over safe Data. Object 0 is the core's storage.
use super::private_data::Core;
use obs_core::private_data;

#[test]
fn null_settings_before_startup() {
    // With no settings the core's storage is never read, so a missing core
    // is never touched: no host call at all.
    let mut core = Core::new();
    core.objects.clear();
    private_data::apply(&mut core, None);
    assert!(core.calls.is_empty());
    assert!(core.held.is_empty());
}

#[test]
fn apply_to_empty_storage_copies_values() {
    let mut core = Core::new();
    let held = private_data::get(&mut core);
    assert!(core.data(held).first().is_none());
    core.data(1).set_string("name", Some("original"));
    core.data(1).set_int("count", 42);
    private_data::apply(&mut core, Some(1));

    let read = private_data::get(&mut core);
    assert_eq!(read, held);
    assert_ne!(read, 1);
    assert_eq!(core.data(read).get_string("name"), "original");
    assert_eq!(core.data(read).get_int("count"), 42);
    core.data(1).set_string("name", Some("caller changed"));
    core.data(held).set_int("count", 99);
    assert_eq!(core.data(held).get_string("name"), "original");
    assert_eq!(core.data(1).get_int("count"), 42);
    core.objects[1] = None;
    assert_eq!(core.data(held).get_string("name"), "original");
    core.release(read);
    core.release(held);
}

#[test]
fn merge_preserves_storage_and_unrelated_keys() {
    let mut core = Core::new();
    let held = private_data::get(&mut core);
    core.data(held).set_string("old-only", Some("keep"));
    core.data(held).set_string("shared", Some("old"));
    core.data(held).set_default_int("default-only", 7);
    core.data(1).set_int("shared", 42);
    core.data(1).set_bool("new-only", true);
    private_data::apply(&mut core, Some(1));

    let read = private_data::get(&mut core);
    assert_eq!(read, held);
    assert_ne!(read, 1);
    assert_eq!(core.data(held).get_string("old-only"), "keep");
    assert_eq!(core.data(held).get_int("shared"), 42);
    assert!(core.data(held).get_bool("new-only"));
    assert_eq!(core.data(held).get_int("default-only"), 7);
    assert!(!core.data(held).has_user_value("default-only"));
    assert!(!core.data(1).has_user_value("old-only"));
    assert_eq!(core.data(1).get_int("shared"), 42);
    core.objects[1] = None;
    core.release(read);
    core.release(held);
}

#[test]
fn null_and_empty_settings_preserve_values() {
    let mut core = Core::new();
    let held = private_data::get(&mut core);
    core.data(held).set_string("keep", Some("unchanged"));
    core.data(held).set_default_int("default", 8);
    private_data::apply(&mut core, None);
    assert_eq!(core.data(held).get_string("keep"), "unchanged");
    assert_eq!(core.data(held).get_int("default"), 8);
    private_data::apply(&mut core, Some(2));
    core.objects[2] = None;
    let read = private_data::get(&mut core);
    assert_eq!(read, held);
    assert_eq!(core.data(read).get_string("keep"), "unchanged");
    assert_eq!(core.data(read).get_int("default"), 8);
    core.release(read);
    core.release(held);
}

#[test]
fn applied_values_survive_core_shutdown() {
    let mut core = Core::new();
    let held = private_data::get(&mut core);
    core.data(1).set_string("retained", Some("after shutdown"));
    private_data::apply(&mut core, Some(1));
    core.objects[1] = None;
    // Shutdown drops the core's own reference; the caller's retain survives.
    core.objects[0] = None;
    assert_eq!(core.data(held).get_string("retained"), "after shutdown");
    core.release(held);
}

#[test]
fn dependency_calls_follow_c_order() {
    // No clear: apply merges. Settings are checked before the storage read.
    let mut core = Core::new();
    private_data::apply(&mut core, None);
    assert!(core.calls.is_empty());
    private_data::apply(&mut core, Some(1));
    assert_eq!(core.calls, ["read", "apply"]);
    core.calls.clear();
    private_data::apply(&mut core, Some(0));
    assert_eq!(core.calls, ["read", "apply"]);
}
