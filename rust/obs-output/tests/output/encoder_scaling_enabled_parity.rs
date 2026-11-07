//! Exact public getter parity, real-header layout and whole-encoder immutability.
use core::ffi::c_void;
use core::mem::{align_of, offset_of, size_of};
use obs_output::ffi::encoder_scaling_enabled::{ScalingFields, obs_encoder_scaling_enabled};
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_encoder_scaling_layout(layout: *mut usize);
    fn oracle_encoder_scaling_new(width: u32, height: u32, video: bool) -> *mut c_void;
    fn oracle_encoder_scaling_free(encoder: *mut c_void);
    fn oracle_encoder_scaling_unchanged(encoder: *mut c_void) -> bool;
    fn oracle_obs_encoder_scaling_enabled(encoder: *const c_void) -> bool;
    fn oracle_encoder_scaling_null_log(getter: unsafe extern "C" fn(*const c_void) -> bool)
    -> bool;
}

#[test]
fn real_header_layout() {
    let mut layout = [0; 4];
    // SAFETY: the oracle writes exactly four size_t values.
    unsafe { oracle_encoder_scaling_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<ScalingFields>(),
            align_of::<ScalingFields>(),
            offset_of!(ScalingFields, width),
            offset_of!(ScalingFields, height)
        ]
    );
}

struct Encoder(*mut c_void);
impl Drop for Encoder {
    fn drop(&mut self) {
        // SAFETY: this wrapper owns the fixture, released exactly once.
        unsafe { oracle_encoder_scaling_free(self.0) };
    }
}
fn check(width: u32, height: u32, video: bool) {
    // SAFETY: the fixture constructor initializes the real private C layout.
    let encoder = Encoder(unsafe { oracle_encoder_scaling_new(width, height, video) });
    assert!(!encoder.0.is_null());
    // SAFETY: the exclusive fixture stays live for both read-only getters.
    unsafe {
        assert_eq!(
            obs_encoder_scaling_enabled(encoder.0),
            oracle_obs_encoder_scaling_enabled(encoder.0)
        );
        assert_eq!(
            obs_encoder_scaling_enabled(encoder.0),
            width != 0 || height != 0
        );
        assert!(oracle_encoder_scaling_unchanged(encoder.0));
    }
}
#[test]
fn characterized_edges() {
    for width in [0, 1, 1920, 0x8000_0000, u32::MAX] {
        for height in [0, 1, 1920, 0x8000_0000, u32::MAX] {
            for video in [false, true] {
                check(width, height, video);
            }
        }
    }
}
#[test]
fn null_default_and_logs() {
    if !super::isolated(concat!(module_path!(), "::null_default_and_logs")) {
        return;
    }
    // SAFETY: each callback accepts null; logger changes are isolated and restored.
    unsafe {
        assert!(oracle_encoder_scaling_null_log(
            oracle_obs_encoder_scaling_enabled
        ));
        assert!(oracle_encoder_scaling_null_log(obs_encoder_scaling_enabled));
    }
}
proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn random_axes_match_c(width in any::<u32>(), height in any::<u32>(), video in any::<bool>()) {
        check(width, height, video);
    }
}
