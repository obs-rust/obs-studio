//! Exact C body parity, queue allocation preservation and mutex scope; no exclusions.
use core::ffi::{c_long, c_void};
use obs_scene::ffi::set_visibility::obs_sceneitem_reconcile_visibility;
use proptest::prelude::*;
use std::sync::Mutex;

// The C call recorder is shared; only these probes use it.
static PROBE: Mutex<()> = Mutex::new(());
unsafe extern "C" {
    fn oracle_scene_item_set_visibility_probe(
        refs: c_long,
        bits: u32,
        queued: u32,
        reconcile: unsafe extern "C" fn(*mut c_void, bool),
    ) -> u32;
}
fn check(refs: c_long, bits: u32, queued: u32) {
    let _guard = PROBE.lock().unwrap();
    // SAFETY: C supplies a complete initialized item and mutex, then frees all fixture storage.
    assert_eq!(
        unsafe {
            oracle_scene_item_set_visibility_probe(
                refs,
                bits,
                queued,
                obs_sceneitem_reconcile_visibility,
            )
        },
        0,
        "refs={refs} bits={bits} queued={queued}"
    );
}
#[test]
fn characterized_edges() {
    for refs in [c_long::MIN, -1, 0, 1, 2, c_long::MAX] {
        for bits in 0..8 {
            for queued in [0, 1, 4, 8] {
                check(refs, bits, queued);
            }
        }
    }
}
proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(128), failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn matches_original_c(refs in any::<c_long>(), bits in any::<u32>(), queued in 0u32..9) {
        check(refs, bits, queued);
    }
}
