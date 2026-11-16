//! Real-header layout, mutex ordering, retained storage and C parity; no differences.
#![allow(unsafe_code)]
use core::ffi::c_void;
use core::mem::{align_of, offset_of, size_of};
use obs_output::ffi::encoder_clear_roi::{ClearRoiFields, obs_encoder_clear_roi};
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_obs_encoder_clear_roi(encoder: *mut c_void);
    fn oracle_encoder_clear_roi_new(
        count: usize,
        increment: u32,
        empty_on_lock: bool,
    ) -> *mut c_void;
    fn oracle_encoder_clear_roi_free(encoder: *mut c_void);
    fn oracle_encoder_clear_roi_check(encoder: *mut c_void, before: usize, increment: u32) -> bool;
    fn oracle_encoder_clear_roi_layout(layout: *mut usize);
    fn oracle_encoder_clear_roi_null_log(clear: unsafe extern "C" fn(*mut c_void)) -> bool;
}
struct Fixture(*mut c_void);
impl Drop for Fixture {
    fn drop(&mut self) {
        // SAFETY: fixture owns this matching C allocation and initialized mutex.
        unsafe { oracle_encoder_clear_roi_free(self.0) };
    }
}
fn check(count: usize, increment: u32, empty_on_lock: bool) {
    for clear in [oracle_obs_encoder_clear_roi, obs_encoder_clear_roi] {
        // SAFETY: complete C fixture; no array elements are dereferenced by clear.
        let fixture =
            Fixture(unsafe { oracle_encoder_clear_roi_new(count, increment, empty_on_lock) });
        assert!(!fixture.0.is_null());
        // SAFETY: live, exclusively owned C encoder with its actual shared mutex.
        unsafe {
            clear(fixture.0);
            assert!(oracle_encoder_clear_roi_check(fixture.0, count, increment));
            // A second empty clear must neither lock nor increment.
            clear(fixture.0);
            assert!(oracle_encoder_clear_roi_check(fixture.0, count, increment));
        }
    }
}
#[test]
fn layout_matches_real_header() {
    let mut layout = [usize::MAX; 7];
    // SAFETY: C writes exactly seven size_t values to this array.
    unsafe { oracle_encoder_clear_roi_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<ClearRoiFields>(),
            align_of::<ClearRoiFields>(),
            offset_of!(ClearRoiFields, count),
            offset_of!(ClearRoiFields, increment),
            offset_of!(ClearRoiFields, mutex),
            offset_of!(ClearRoiFields, lock),
            offset_of!(ClearRoiFields, unlock)
        ]
    );
}
#[test]
fn characterized_edges() {
    for count in [0, 1, 2, usize::MAX] {
        for increment in [0, 1, 0x8000_0001, u32::MAX] {
            for empty in [false, true] {
                check(count, increment, empty);
            }
        }
    }
}
#[test]
fn exact_null_diagnostics() {
    if !super::isolated(concat!(module_path!(), "::exact_null_diagnostics")) {
        return;
    }
    // SAFETY: the isolated child exclusively owns the process-wide log handler.
    unsafe {
        assert!(oracle_encoder_clear_roi_null_log(
            oracle_obs_encoder_clear_roi
        ));
        assert!(oracle_encoder_clear_roi_null_log(obs_encoder_clear_roi));
    }
}
proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256), failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn random_state_matches_c(count in any::<usize>(), increment in any::<u32>(), empty in any::<bool>()) {
        check(count, increment, empty);
    }
}
