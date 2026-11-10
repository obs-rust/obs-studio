//! Tier 3 original-C parity for #4470; no input exclusions.
use core::ffi::c_void;
use obs_output::ffi::output_video_encoder_getter::obs_output_get_video_encoder2 as video_encoder;
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

type Getter = unsafe extern "C" fn(*const c_void, usize) -> *mut c_void;
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);
unsafe extern "C" {
    fn oracle_output_video_encoder_new(occupied: u32, flags: u32) -> *mut c_void;
    fn oracle_output_video_encoder_expected(output: *const c_void, idx: usize) -> *mut c_void;
    fn oracle_output_video_encoder_free(output: *mut c_void);
    fn oracle_output_video_encoder_unchanged(output: *const c_void) -> bool;
    fn oracle_obs_output_get_video_encoder2(output: *const c_void, idx: usize) -> *mut c_void;
    fn oracle_output_video_encoder_logs(
        getter: Getter,
        get: GetLog,
        set: SetLog,
        idx: usize,
    ) -> bool;
}

struct Fixture(*mut c_void);
impl Drop for Fixture {
    fn drop(&mut self) {
        // SAFETY: owns the C allocation, including all encoder tokens.
        unsafe { oracle_output_video_encoder_free(self.0) }
    }
}

fn check(occupied: u32, flags: u32, idx: usize) {
    // SAFETY: the oracle allocates the real output and encoder layouts.
    let fixture = Fixture(unsafe { oracle_output_video_encoder_new(occupied, flags) });
    assert!(!fixture.0.is_null());
    // SAFETY: the live fixture owns its encoder tokens. NULL is never supplied
    // to an accessor that takes a member address.
    unsafe {
        for _ in 0..2 {
            assert_eq!(
                video_encoder(fixture.0, idx),
                oracle_obs_output_get_video_encoder2(fixture.0, idx)
            );
            assert_eq!(
                video_encoder(fixture.0, idx),
                oracle_output_video_encoder_expected(fixture.0, idx)
            );
            assert!(oracle_output_video_encoder_unchanged(fixture.0));
        }
    }
}

#[test]
fn characterized_edges() {
    for occupied in [0, 1, 2, 1 << 9, u32::MAX] {
        for flags in [0, u32::MAX] {
            for idx in (0..=11).chain([usize::MAX / 2, usize::MAX - 1, usize::MAX]) {
                check(occupied, flags, idx);
            }
        }
    }
}

#[test]
fn null_diagnostics_match_c() {
    if !super::isolated(concat!(module_path!(), "::null_diagnostics_match_c")) {
        return;
    }
    // SAFETY: this isolated process owns the log handler; the probe restores it.
    unsafe {
        for idx in [0, 9, 10, usize::MAX] {
            assert!(oracle_output_video_encoder_logs(
                video_encoder,
                base_get_log_handler,
                base_set_log_handler,
                idx
            ));
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(128), failure_persistence: None, .. ProptestConfig::default()
    })]
    #[test]
    fn borrowed_encoder_matches_c(occupied in any::<u32>(), flags in any::<u32>(), idx in prop_oneof![0_usize..12, any::<usize>()]) { check(occupied, flags, idx); }
}
