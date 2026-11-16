//! Real-header layout, exact getter parity and whole-encoder immutability.
use core::ffi::{c_int, c_void};
use core::mem::{align_of, offset_of, size_of};
use obs_output::ffi::encoder_preferred_color_space::{
    PreferredColorSpaceFields, obs_encoder_get_preferred_color_space,
};
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_encoder_preferred_color_space_layout(layout: *mut usize);
    fn oracle_encoder_preferred_color_space_new(kind: c_int, space: u32) -> *mut c_void;
    fn oracle_encoder_preferred_color_space_unchanged(encoder: *mut c_void) -> bool;
    fn oracle_encoder_preferred_color_space_free(encoder: *mut c_void);
    fn oracle_obs_encoder_get_preferred_color_space(encoder: *const c_void) -> u32;
}

#[test]
fn real_header_layout() {
    let mut layout = [0; 6];
    // SAFETY: the oracle writes exactly six size_t entries.
    unsafe { oracle_encoder_preferred_color_space_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<PreferredColorSpaceFields>(),
            align_of::<PreferredColorSpaceFields>(),
            offset_of!(PreferredColorSpaceFields, kind),
            offset_of!(PreferredColorSpaceFields, space),
            size_of::<c_int>(),
            size_of::<u32>()
        ]
    );
}

struct Encoder(*mut c_void);
impl Drop for Encoder {
    fn drop(&mut self) {
        // SAFETY: the fixture belongs to this wrapper and is freed exactly once.
        unsafe { oracle_encoder_preferred_color_space_free(self.0) };
    }
}
fn check(kind: i32, space: u32) {
    // SAFETY: constructor initializes the real encoder layout without external resources.
    let encoder = Encoder(unsafe { oracle_encoder_preferred_color_space_new(kind, space) });
    assert!(!encoder.0.is_null());
    // SAFETY: both getters borrow the same live fixture without changing it.
    unsafe {
        assert_eq!(
            obs_encoder_get_preferred_color_space(encoder.0),
            oracle_obs_encoder_get_preferred_color_space(encoder.0)
        );
        assert!(oracle_encoder_preferred_color_space_unchanged(encoder.0));
    }
}

#[test]
fn characterized_edges_and_null() {
    // SAFETY: both original and replacement explicitly accept NULL.
    unsafe {
        assert_eq!(
            oracle_obs_encoder_get_preferred_color_space(core::ptr::null()),
            0
        );
        assert_eq!(obs_encoder_get_preferred_color_space(core::ptr::null()), 0);
    }
    for kind in [0, 1, 2, -1, i32::MIN, i32::MAX] {
        for space in [0, 1, 2, 3, 4, 5, 6, i32::MAX as u32, 0x8000_0000, u32::MAX] {
            check(kind, space);
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
    fn random_values_match_c(kind in prop_oneof![Just(0i32), Just(1i32), any::<i32>()], space in any::<u32>()) {
        check(kind, space);
    }
}
