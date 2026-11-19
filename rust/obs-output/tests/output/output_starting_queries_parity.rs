//! Whole-original-C differential coverage for #5233, with no exclusions.
//! No new ABI layout: the output is opaque and adapters return scalar bools.
use core::ffi::{c_uint, c_void};
use obs_output::ffi::output_starting_queries::{obs_output_needs_encoders, obs_output_starting};
use proptest::prelude::*;

type Query = unsafe extern "C" fn(*const c_void) -> bool;
unsafe extern "C" {
    fn oracle_output_starting_check(
        candidate: Query,
        mask: c_uint,
        starting: bool,
        flip: bool,
    ) -> bool;
}
fn check(mask: c_uint, starting: bool, flip: bool) {
    let candidate = if starting {
        obs_output_starting
    } else {
        obs_output_needs_encoders
    };
    // SAFETY: oracle owns the real initialized output and records each borrowed
    // pointer before freeing it; candidate retains no references after returning.
    assert!(unsafe { oracle_output_starting_check(candidate, mask, starting, flip) });
}
#[test]
fn all_states_preserve_real_addresses_read_order_and_storage() {
    for mask in 0..16 {
        for starting in [false, true] {
            for flip in [false, true] {
                check(mask, starting, flip);
            }
        }
    }
}
#[test]
fn null_rejected_before_any_c_projection() {
    // SAFETY: NULL is rejected by each shim before invoking a C adapter.
    unsafe {
        assert!(!obs_output_needs_encoders(core::ptr::null()));
        assert!(!obs_output_starting(core::ptr::null()));
    }
}
proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(128),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn arbitrary_flags_match_original_c(mask in any::<c_uint>(), starting in any::<bool>(), flip in any::<bool>()) {
        check(mask, starting, flip);
    }
}
