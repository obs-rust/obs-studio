//! Real-header prefix layout and exact Tier 3 parity; no intended differences.
//! C owns complete items. The null log probe runs in an isolated child.
use core::mem::{align_of, offset_of, size_of};
use obs_c_oracle as _;
use obs_scene::ffi::item_id::{SceneItemIdPrefix, obs_sceneitem_get_id};
use proptest::prelude::*;

type Getter = unsafe extern "C" fn(*const SceneItemIdPrefix) -> i64;
unsafe extern "C" {
    fn oracle_scene_item_id_layout(field: u32) -> usize;
    fn oracle_scene_item_id_probe(flags: u32, id: i64, getter: Getter) -> u32;
    fn oracle_scene_item_id_null_probe(getter: Getter) -> u32;
}

#[test]
fn real_header_prefix_layout() {
    let rust = [
        size_of::<SceneItemIdPrefix>(),
        align_of::<SceneItemIdPrefix>(),
        offset_of!(SceneItemIdPrefix, refs),
        offset_of!(SceneItemIdPrefix, removed),
        offset_of!(SceneItemIdPrefix, is_group),
        offset_of!(SceneItemIdPrefix, is_scene),
        offset_of!(SceneItemIdPrefix, update_transform),
        offset_of!(SceneItemIdPrefix, update_group_resize),
        offset_of!(SceneItemIdPrefix, id),
    ];
    for (index, value) in rust.into_iter().enumerate() {
        // SAFETY: the oracle accepts every integer and returns a layout value.
        assert_eq!(
            value,
            unsafe { oracle_scene_item_id_layout(index as u32) },
            "layout index {index}"
        );
    }
}

fn check(flags: u32, id: i64) {
    // SAFETY: C constructs a complete live item and calls the getter
    // synchronously with its initialized ID, then compares all item bytes.
    let mismatches = unsafe { oracle_scene_item_id_probe(flags, id, obs_sceneitem_get_id) };
    assert_eq!(
        mismatches, 0,
        "flags={flags:#x}, id={id}, mismatch bits={mismatches}"
    );
}

#[test]
fn characterized_edges() {
    for flags in 0..128 {
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
            check(flags, id);
        }
    }
}

#[test]
fn null_diagnostic() {
    const CHILD: &str = "OBS_SCENE_ITEM_ID_LOG_CHILD";
    if std::env::var_os(CHILD).is_some() {
        // SAFETY: this child has no concurrent tests; the C probe saves and
        // restores the global log handler around synchronous getter calls.
        assert_eq!(
            unsafe { oracle_scene_item_id_null_probe(obs_sceneitem_get_id) },
            0
        );
        return;
    }
    let module = module_path!().split_once("::").unwrap().1;
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .env(CHILD, "1")
        .args([
            "--exact",
            &format!("{module}::null_diagnostic"),
            "--nocapture",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
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

// NULL is excluded: the unchanged C setter dereferences its item unconditionally.
type Setter = unsafe extern "C" fn(*mut SceneItemIdPrefix, i64);
unsafe extern "C" {
    fn oracle_scene_item_set_id_probe(flags: u32, initial: i64, id: i64, setter: Setter) -> u32;
}

fn check_set(flags: u32, initial: i64, id: i64) {
    // SAFETY: C owns complete live items, grants exclusive ID access during
    // each callback, and compares every byte against the unchanged C setter.
    let mismatches = unsafe {
        oracle_scene_item_set_id_probe(
            flags,
            initial,
            id,
            obs_scene::ffi::item_id::obs_sceneitem_set_id,
        )
    };
    assert_eq!(
        mismatches, 0,
        "flags={flags:#x}, initial={initial}, id={id}"
    );
}

#[test]
fn set_id_characterized_edges() {
    for flags in 0..128 {
        for id in [
            i64::MIN,
            i64::MIN + 1,
            -17,
            -1,
            0,
            1,
            4242,
            i64::MAX - 1,
            i64::MAX,
        ] {
            check_set(flags, id.wrapping_add(1), id);
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
    fn set_id_arbitrary(flags in any::<u32>(), initial in any::<i64>(), id in any::<i64>()) {
        check_set(flags, initial, id);
    }
}
