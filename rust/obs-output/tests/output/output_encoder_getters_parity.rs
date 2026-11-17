//! Tier 3: the unchanged extracted getters (`obs-output-encoder-getters.c.h`)
//! against the Rust ABI, both over the real private output layout through
//! the production field adapter. The C side calls the original
//! `obs_output_get_video_encoder2()` and slot rules of their own oracles; the
//! Rust side calls the ported ones. No intentional differences; no input
//! exclusions.
use core::ffi::c_void;
use obs_output::ffi::output_encoder_getters::{
    obs_output_get_audio_encoder as audio_encoder, obs_output_get_video_encoder as video_encoder,
};
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

type Video = unsafe extern "C" fn(*const c_void) -> *mut c_void;
type Audio = unsafe extern "C" fn(*const c_void, usize) -> *mut c_void;
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);
unsafe extern "C" {
    fn oracle_output_encoder_getters_new(video: u32, audio: u32, flags: u32) -> *mut c_void;
    fn oracle_output_encoder_getters_expected_video(output: *const c_void) -> *mut c_void;
    fn oracle_output_encoder_getters_expected_audio(
        output: *const c_void,
        idx: usize,
    ) -> *mut c_void;
    fn oracle_output_encoder_getters_unchanged(output: *const c_void) -> bool;
    fn oracle_output_encoder_getters_free(output: *mut c_void);
    fn oracle_obs_output_get_video_encoder(output: *const c_void) -> *mut c_void;
    fn oracle_obs_output_get_audio_encoder(output: *const c_void, idx: usize) -> *mut c_void;
    fn oracle_output_encoder_getters_logs(
        video: Video,
        audio: Audio,
        get: GetLog,
        set: SetLog,
        idx: usize,
    ) -> bool;
}

struct Fixture(*mut c_void);
impl Drop for Fixture {
    fn drop(&mut self) {
        // SAFETY: owns the C allocation, including all encoder tokens.
        unsafe { oracle_output_encoder_getters_free(self.0) }
    }
}

fn check(video: u32, audio: u32, flags: u32, idx: usize) {
    // SAFETY: the oracle allocates the real output and encoder layouts.
    let fixture = Fixture(unsafe { oracle_output_encoder_getters_new(video, audio, flags) });
    assert!(!fixture.0.is_null());
    // SAFETY: the live fixture owns its encoder tokens and is never NULL here.
    unsafe {
        for _ in 0..2 {
            let r = video_encoder(fixture.0);
            assert_eq!(r, oracle_obs_output_get_video_encoder(fixture.0));
            assert_eq!(r, oracle_output_encoder_getters_expected_video(fixture.0));
            let r = audio_encoder(fixture.0, idx);
            assert_eq!(r, oracle_obs_output_get_audio_encoder(fixture.0, idx));
            assert_eq!(
                r,
                oracle_output_encoder_getters_expected_audio(fixture.0, idx)
            );
            assert!(oracle_output_encoder_getters_unchanged(fixture.0));
        }
    }
}

const INDEXES: [usize; 7] = [0, 1, 5, 6, 7, usize::MAX - 1, usize::MAX];

#[test]
fn characterized_edges() {
    let masks = [0, 1, 2, 1 << 5, 1 << 9, 1 << 12, 0b10_1000, u32::MAX];
    for video in masks {
        for audio in masks {
            for flags in [0, u32::MAX] {
                for idx in INDEXES {
                    check(video, audio, flags, idx);
                }
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
        for idx in INDEXES {
            assert!(oracle_output_encoder_getters_logs(
                video_encoder,
                audio_encoder,
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
    fn borrowed_encoders_match_c(
        video in any::<u32>(),
        audio in any::<u32>(),
        flags in any::<u32>(),
        idx in prop_oneof![0_usize..8, Just(usize::MAX), any::<usize>()],
    ) {
        check(video, audio, flags, idx);
    }
}
