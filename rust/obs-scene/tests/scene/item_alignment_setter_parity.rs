//! Tier 3: exact original C versus the shim, whole-item bytes and update events.
//! No intentional differences. Real transform dispatch stays covered by Tier 2.
use core::mem::offset_of;
use obs_scene::ffi::{
    item_alignment_setter::obs_sceneitem_set_alignment, item_bounds_getters::SceneItemPrefix,
};
use proptest::prelude::*;

type Setter = unsafe extern "C" fn(*mut SceneItemPrefix, u32);
unsafe extern "C" {
    fn oracle_scene_alignment_probe(
        setter: Setter,
        old: u32,
        value: u32,
        defer: i32,
        flags: u32,
    ) -> u32;
    fn oracle_scene_alignment_null(setter: Setter) -> u32;
    fn oracle_scene_item_bounds_layout(field: u32) -> usize;
}

#[test]
fn alignment_field_uses_real_header_layout() {
    // SAFETY: a pure header measurement, also fully checked by bounds getter layout tests.
    assert_eq!(offset_of!(SceneItemPrefix, align), unsafe {
        oracle_scene_item_bounds_layout(10)
    });
}

fn check(old: u32, value: u32, defer: i32, flags: u32) {
    // SAFETY: the oracle owns complete live items and scenes; callbacks obey their lifetime.
    let result = unsafe {
        oracle_scene_alignment_probe(obs_sceneitem_set_alignment, old, value, defer, flags)
    };
    assert_eq!(
        result, 0,
        "old={old:#x} value={value:#x} defer={defer} flags={flags}"
    );
}

#[test]
fn characterized_bits_defers_and_unchanged_values() {
    for flags in 0..8 {
        for value in [0, 1 | 4, 2 | 8, 0x8000_0000, u32::MAX] {
            for defer in [i32::MIN, -1, 0, 1, 3, i32::MAX] {
                check(!value, value, defer, flags);
                check(value, value, defer, flags);
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(128), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn all_alignment_bits_and_defer_counts(old in any::<u32>(), value in any::<u32>(), defer in any::<i32>(), flags in 0..8u32) {
        check(old, value, defer, flags);
    }
}

#[test]
fn null_is_silent_in_an_isolated_process() {
    const CHILD: &str = "OBS_ALIGNMENT_NULL_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let name = format!(
            "{}::null_is_silent_in_an_isolated_process",
            module_path!().split_once("::").unwrap().1
        );
        assert!(
            std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", &name, "--nocapture"])
                .env(CHILD, "1")
                .status()
                .unwrap()
                .success()
        );
        return;
    }
    // SAFETY: both setters accept NULL; process isolation protects the logging callback.
    assert_eq!(
        unsafe { oracle_scene_alignment_null(obs_sceneitem_set_alignment) },
        0
    );
}
