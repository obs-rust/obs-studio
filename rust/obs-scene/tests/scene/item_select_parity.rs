//! Original C setter parity over complete C-owned items and real signal handlers.
//! Reuses SceneItemPrefix's full real-header layout test in item_state_getters_parity.
//! No intended behavior differences or excluded inputs.
use obs_scene::ffi::item_select::obs_sceneitem_select;
use obs_scene::ffi::item_state_getters::SceneItemPrefix;
use proptest::prelude::*;

type Setter = unsafe extern "C" fn(*mut SceneItemPrefix, bool);
unsafe extern "C" {
    fn oracle_scene_item_select_probe(flags: u32, requests: u32, id: i64, setter: Setter) -> u32;
}

fn check(flags: u32, requests: u32, id: i64) {
    // SAFETY: C constructs complete items, parents and real signal handlers,
    // calls both setters synchronously, compares state and frees the handlers.
    assert_eq!(
        unsafe { oracle_scene_item_select_probe(flags, requests, id, obs_sceneitem_select) },
        0,
        "flags={flags:#x}, requests={requests:#x}, id={id}"
    );
}

#[test]
fn characterized_edges() {
    for flags in 0..64 {
        for requests in [0, u32::MAX, 0xaaaa, 0x5555] {
            for id in [i64::MIN, -17, 0, i64::MAX] {
                check(flags, requests, id);
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
    fn selection_sequence_matches_c(flags in any::<u32>(), requests in any::<u32>(), id in any::<i64>()) {
        check(flags, requests, id);
    }
}
