//! Real C-owned layout, complete item bytes and ordered reference traces.
//! No intentional differences; generated allocation sizes are fixed in C.
use core::ffi::c_void;
use obs_scene::ffi::item_transition_setters::{
    Calls, obs_sceneitem_set_transition_duration, set_transition_with,
};
use proptest::prelude::*;

type Transition = unsafe extern "C" fn(*mut c_void, bool, *mut c_void);
type Duration = unsafe extern "C" fn(*mut c_void, bool, u32);
unsafe extern "C" {
    fn oracle_scene_setter_release(source: *mut c_void);
    fn oracle_scene_setter_get_ref(source: *mut c_void) -> *mut c_void;
    fn oracle_scene_setter_compare(
        transition: Transition,
        duration: Duration,
        show: bool,
        flags: u8,
        replacement: u8,
        ms: u32,
    ) -> bool;
}
enum Recorded {}
impl Calls for Recorded {
    unsafe fn release(source: *mut c_void) {
        // SAFETY: the C fixture owns this live source sentinel and records calls.
        unsafe { oracle_scene_setter_release(source) }
    }
    unsafe fn get_ref(source: *mut c_void) -> *mut c_void {
        // SAFETY: the C fixture owns all sentinels and accepts NULL.
        unsafe { oracle_scene_setter_get_ref(source) }
    }
}
unsafe extern "C" fn transition(item: *mut c_void, show: bool, source: *mut c_void) {
    // SAFETY: C owns the real-layout mutable item and source sentinels.
    unsafe { set_transition_with::<Recorded>(item, show, source) }
}
fn check(show: bool, flags: u8, replacement: u8, ms: u32) {
    // SAFETY: C creates and frees complete objects and invokes callbacks only
    // while live; its recording reference model does not destroy their storage.
    assert!(
        unsafe {
            oracle_scene_setter_compare(
                transition,
                obs_sceneitem_set_transition_duration,
                show,
                flags,
                replacement,
                ms,
            )
        },
        "show={show} flags={flags} replacement={replacement} ms={ms}"
    );
}
#[test]
fn reference_order_routing_and_duration_edges_match_c() {
    for show in [false, true] {
        for flags in 0..32 {
            for replacement in 0..4 {
                for ms in [0, 1, 250, 750, u32::MAX] {
                    check(show, flags, replacement, ms);
                }
            }
        }
    }
}
proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(512), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn setters_match_c(show in any::<bool>(), flags in 0u8..32, replacement in 0u8..4, ms in any::<u32>()) {
        check(show, flags, replacement, ms);
    }
}
