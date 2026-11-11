//! Tier 3: original C wrapper, exact delegation and unchanged private state.
//! No exclusions; snapshot/removal semantics stay in the existing C enumerator.
use core::ffi::{c_long, c_void};
use obs_scene::ffi::group_enum::{SceneCallback, obs_sceneitem_group_enum_items};
use proptest::prelude::*;

type Port = unsafe extern "C" fn(*mut c_void, Option<SceneCallback>, *mut c_void);
unsafe extern "C" {
    fn oracle_scene_group_enum_probe(bits: u32, id: c_long, port: Port) -> u32;
}

fn check(bits: u32, id: i32) {
    // SAFETY: C owns complete live items, sources, scenes, callback and param
    // for both calls. Instrumentation is isolated in thread-local storage.
    let result = unsafe {
        oracle_scene_group_enum_probe(bits, c_long::from(id), obs_sceneitem_group_enum_items)
    };
    assert_eq!(result, 0, "bits={bits:#x}, id={id}");
}

#[test]
fn characterized_edges() {
    for bits in 0..4096 {
        check(bits, -17);
    }
    for id in [i32::MIN, -1, 0, 1, i32::MAX] {
        check(1 | 8 | (3 << 5) | 512, id);
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(128), failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn matches_original_c(bits in any::<u32>(), id in any::<i32>()) {
        check(bits, id);
    }
}
