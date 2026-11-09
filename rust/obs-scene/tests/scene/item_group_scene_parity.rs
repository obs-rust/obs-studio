//! Exact borrowed-pointer and whole-object parity; no exclusions.
use core::ffi::c_void;
use obs_scene::ffi::item_group_scene::obs_sceneitem_group_get_scene;
use proptest::prelude::*;

type Getter = unsafe extern "C" fn(*const c_void) -> *mut c_void;
unsafe extern "C" {
    fn oracle_scene_item_group_scene_probe(bits: u32, getter: Getter) -> u32;
}

fn check(bits: u32) {
    // SAFETY: C owns complete live items, sources and scenes for the callback.
    assert_eq!(
        unsafe { oracle_scene_item_group_scene_probe(bits, obs_sceneitem_group_get_scene) },
        0,
        "bits={bits:#x}"
    );
}

#[test]
fn characterized_edges() {
    for bits in 0..32 {
        check(bits);
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(128), failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn group_scene_matches_c(bits in any::<u32>()) {
        check(bits);
    }
}
