//! Exact original-C parity and byte-for-byte item preservation; no exclusions.
use core::ffi::c_void;
use obs_scene::ffi::item_visible::obs_sceneitem_visible;
use proptest::prelude::*;

type Getter = unsafe extern "C" fn(*const c_void) -> bool;
unsafe extern "C" {
    fn oracle_scene_item_visible_probe(bits: u32, getter: Getter) -> u32;
}

fn check(bits: u32) {
    // SAFETY: C owns a complete stack item and calls the getter only while it is live.
    assert_eq!(
        unsafe { oracle_scene_item_visible_probe(bits, obs_sceneitem_visible) },
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
    fn user_visibility_matches_c(bits in any::<u32>()) {
        check(bits);
    }
}
