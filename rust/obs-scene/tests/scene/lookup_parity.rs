//! Exact pointer and mutex-order parity against the unchanged extracted C.
//! Null name inputs to direct/recursive lookup with a non-null scene are C UB
//! and excluded. Group lookup's null names remain covered. No behavior changes.
use core::ffi::{c_char, c_void};
use obs_scene::ffi::lookup::{
    obs_scene_find_sceneitem_by_id, obs_scene_find_source, obs_scene_find_source_recursive,
    obs_scene_get_group,
};
use proptest::prelude::*;
use std::sync::Mutex;

type Name = unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_void;
type Id = unsafe extern "C" fn(*mut c_void, i64) -> *mut c_void;
unsafe extern "C" {
    fn oracle_scene_lookup_probe(
        seed: u32,
        id: i64,
        direct: Name,
        recursive: Name,
        by_id: Id,
        group: Name,
    ) -> u32;
}
// The oracle's lock recorder is shared by calls in this module only.
static PROBE: Mutex<()> = Mutex::new(());
fn check(seed: u32, id: i64) {
    let _guard = PROBE.lock().unwrap();
    // SAFETY: C constructs real live scenes, owns their mutexes and calls these
    // shims only while each fixture and every terminated name remain valid.
    let errors = unsafe {
        oracle_scene_lookup_probe(
            seed,
            id,
            obs_scene_find_source,
            obs_scene_find_source_recursive,
            obs_scene_find_sceneitem_by_id,
            obs_scene_get_group,
        )
    };
    assert_eq!(errors, 0, "seed={seed:#x}, id={id}");
}
#[test]
fn lookup_edges() {
    for seed in [
        0,
        1,
        2,
        3,
        0xfff,
        0x5555,
        0xaaaa,
        u32::MAX,
        1 << 30,
        1 << 31,
    ] {
        for id in [i64::MIN, -1, 0, 1, i64::MAX] {
            check(seed, id);
        }
    }
}
proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256), failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn lookup_matches_original_c(seed in any::<u32>(), id in any::<i64>()) {
        check(seed, id);
    }
}
