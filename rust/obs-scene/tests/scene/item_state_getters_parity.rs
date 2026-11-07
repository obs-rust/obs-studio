//! Tier 2 prefix layout and Tier 3 exact parity. No intended differences.
//! C owns complete real-header items, including the opaque suffix; callbacks
//! exercise the actual shims without fabricating a truncated C allocation.
use core::mem::{align_of, offset_of, size_of};
use obs_c_oracle as _;
use obs_scene::ffi::item_state_getters::{
    SceneItemPrefix, obs_sceneitem_locked, obs_sceneitem_selected,
};
use proptest::prelude::*;

type Getter = unsafe extern "C" fn(*const SceneItemPrefix) -> bool;
unsafe extern "C" {
    fn oracle_scene_item_state_layout(field: u32) -> usize;
    fn oracle_scene_item_state_probe(flags: u32, id: i64, selected: Getter, locked: Getter) -> u32;
}

#[test]
fn real_header_prefix_layout() {
    let rust = [
        size_of::<SceneItemPrefix>(),
        align_of::<SceneItemPrefix>(),
        offset_of!(SceneItemPrefix, refs),
        offset_of!(SceneItemPrefix, removed),
        offset_of!(SceneItemPrefix, is_group),
        offset_of!(SceneItemPrefix, is_scene),
        offset_of!(SceneItemPrefix, update_transform),
        offset_of!(SceneItemPrefix, update_group_resize),
        offset_of!(SceneItemPrefix, id),
        offset_of!(SceneItemPrefix, parent),
        offset_of!(SceneItemPrefix, source),
        offset_of!(SceneItemPrefix, active_refs),
        offset_of!(SceneItemPrefix, defer_update),
        offset_of!(SceneItemPrefix, defer_group_resize),
        offset_of!(SceneItemPrefix, user_visible),
        offset_of!(SceneItemPrefix, visible),
        offset_of!(SceneItemPrefix, selected),
        offset_of!(SceneItemPrefix, locked),
    ];
    for (index, value) in rust.into_iter().enumerate() {
        // SAFETY: the oracle accepts any integer and returns a layout value.
        assert_eq!(
            value,
            unsafe { oracle_scene_item_state_layout(index as u32) },
            "layout index {index}"
        );
    }
}

fn check(flags: u32, id: i64) {
    // SAFETY: C constructs full live items and passes their addresses to the
    // callbacks synchronously. Both callbacks read only initialized bools.
    let mismatches = unsafe {
        oracle_scene_item_state_probe(flags, id, obs_sceneitem_selected, obs_sceneitem_locked)
    };
    assert_eq!(
        mismatches, 0,
        "flags={flags:#x}, id={id}, mismatch bits={mismatches}"
    );
}

#[test]
fn characterized_edges() {
    for flags in 0..512 {
        for id in [i64::MIN, -17, 0, 1, i64::MAX] {
            check(flags, id);
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(1024),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn arbitrary_flags_and_id(flags in any::<u32>(), id in any::<i64>()) {
        check(flags, id);
    }
}
