//! Real-header layout and exact differential checks. No intentional differences.
#![allow(unsafe_code)]
use core::ffi::c_void;
use core::mem::{align_of, offset_of, size_of};
use obs_output::ffi::encoder_roi_state::{
    RoiStateFields, obs_encoder_get_roi_increment, obs_encoder_has_roi,
};
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_obs_encoder_has_roi(encoder: *const c_void) -> bool;
    fn oracle_obs_encoder_get_roi_increment(encoder: *const c_void) -> u32;
    fn oracle_encoder_roi_state_layout(layout: *mut usize);
    fn oracle_encoder_roi_state_new(count: usize, increment: u32) -> *mut c_void;
    fn oracle_encoder_roi_state_unchanged(encoder: *mut c_void) -> bool;
    fn oracle_encoder_roi_state_free(encoder: *mut c_void);
    fn oracle_encoder_roi_state_null_log(
        has_roi: unsafe extern "C" fn(*const c_void) -> bool,
        increment: unsafe extern "C" fn(*const c_void) -> u32,
    ) -> bool;
}
struct Fixture(*mut c_void);
impl Drop for Fixture {
    fn drop(&mut self) {
        // SAFETY: the helper frees its matching complete C allocation.
        unsafe { oracle_encoder_roi_state_free(self.0) };
    }
}
fn fixture(count: usize, increment: u32) -> Fixture {
    // SAFETY: the helper stores these values in a complete, real-header encoder.
    let encoder = unsafe { oracle_encoder_roi_state_new(count, increment) };
    assert!(!encoder.is_null());
    Fixture(encoder)
}
fn check(count: usize, increment: u32) {
    let encoder = fixture(count, increment);
    let other = fixture(1, 19);
    for (encoder, count, increment) in [(&encoder, count, increment), (&other, 1, 19)] {
        // SAFETY: the owned C allocation lives through both getter calls; no array is accessed.
        unsafe {
            let c = oracle_obs_encoder_has_roi(encoder.0);
            assert_eq!(c, count > 0);
            assert_eq!(obs_encoder_has_roi(encoder.0), c);
            let c = oracle_obs_encoder_get_roi_increment(encoder.0);
            assert_eq!(c, increment);
            assert_eq!(obs_encoder_get_roi_increment(encoder.0), c);
            assert!(oracle_encoder_roi_state_unchanged(encoder.0));
        }
    }
}
#[test]
fn layout_matches_real_header() {
    let mut c = [usize::MAX; 4];
    // SAFETY: the helper writes exactly four size_t values.
    unsafe { oracle_encoder_roi_state_layout(c.as_mut_ptr()) };
    assert_eq!(
        c,
        [
            size_of::<RoiStateFields>(),
            align_of::<RoiStateFields>(),
            offset_of!(RoiStateFields, count),
            offset_of!(RoiStateFields, increment)
        ]
    );
}
#[test]
fn exact_null_diagnostics() {
    if !super::isolated(concat!(module_path!(), "::exact_null_diagnostics")) {
        return;
    }
    // SAFETY: the isolated process owns its global logging callback exclusively.
    unsafe {
        assert!(oracle_encoder_roi_state_null_log(
            oracle_obs_encoder_has_roi,
            oracle_obs_encoder_get_roi_increment,
        ));
        assert!(oracle_encoder_roi_state_null_log(
            obs_encoder_has_roi,
            obs_encoder_get_roi_increment,
        ));
    }
}
#[test]
fn characterized_edges() {
    for count in [0, 1, 2, usize::MAX] {
        for increment in [0, 1, 0x8000_0001, u32::MAX] {
            check(count, increment);
        }
    }
}
proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn random_state_matches_c(count in any::<usize>(), increment in any::<u32>()) {
        check(count, increment);
    }
}
