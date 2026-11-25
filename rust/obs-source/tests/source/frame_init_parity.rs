//! Tiers 2/3: full real-header layout and original extracted C behavior.
//! No intentional differences. Excludes non-returning bmalloc(0) inputs
//! (unsupported/NONE formats, zero dimensions), as the unchanged C fixture does.
use core::ffi::c_void;
use core::mem::{align_of, offset_of, size_of};
use obs_media_io::video_io::VideoFormat;
use obs_source::ffi::source_frame_init::{obs_source_frame, obs_source_frame_init};
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

type Init = unsafe extern "C" fn(*mut obs_source_frame, i32, u32, u32);
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);
unsafe extern "C" {
    fn oracle_source_frame_init_layout(out: *mut usize);
    fn oracle_source_frame_init_check(
        init: Init,
        format: i32,
        width: u32,
        height: u32,
        seed: u8,
    ) -> u32;
    fn oracle_source_frame_init_logs(init: Init, get: GetLog, set: SetLog) -> bool;
}

#[test]
fn every_public_source_frame_field_matches_the_real_c_header() {
    let mut original = [0; 18];
    // SAFETY: C writes exactly the two layout values and sixteen field offsets.
    unsafe { oracle_source_frame_init_layout(original.as_mut_ptr()) };
    assert_eq!(
        original,
        [
            size_of::<obs_source_frame>(),
            align_of::<obs_source_frame>(),
            offset_of!(obs_source_frame, data),
            offset_of!(obs_source_frame, linesize),
            offset_of!(obs_source_frame, width),
            offset_of!(obs_source_frame, height),
            offset_of!(obs_source_frame, timestamp),
            offset_of!(obs_source_frame, format),
            offset_of!(obs_source_frame, color_matrix),
            offset_of!(obs_source_frame, full_range),
            offset_of!(obs_source_frame, max_luminance),
            offset_of!(obs_source_frame, color_range_min),
            offset_of!(obs_source_frame, color_range_max),
            offset_of!(obs_source_frame, flip),
            offset_of!(obs_source_frame, flags),
            offset_of!(obs_source_frame, trc),
            offset_of!(obs_source_frame, refs),
            offset_of!(obs_source_frame, prev_frame),
        ]
    );
}

fn check(format: i32, width: u32, height: u32, seed: u8) {
    // SAFETY: C owns correctly aligned real-layout frame storage, compares all
    // bytes without interpreting untouched bools, and frees both allocations.
    let mismatch = unsafe {
        oracle_source_frame_init_check(obs_source_frame_init, format, width, height, seed)
    };
    assert_eq!(
        mismatch, 0,
        "format={format}, {width}x{height}, seed={seed}"
    );
}

#[test]
fn test_source_frame_init_all_formats_planes_and_unrelated_bytes() {
    for format in VideoFormat::ALL.into_iter().skip(1) {
        for (width, height) in [(1, 1), (13, 7), (32, 16)] {
            for seed in [0, 0x5a, 0xff] {
                check(format as i32, width, height, seed);
            }
        }
    }
}

#[test]
fn test_source_frame_init_null_diagnostic_matches_c() {
    if !crate::output::isolated(concat!(
        module_path!(),
        "::test_source_frame_init_null_diagnostic_matches_c"
    )) {
        return;
    }
    // SAFETY: the isolated child owns and restores the global log handler.
    unsafe {
        assert!(oracle_source_frame_init_logs(
            obs_source_frame_init,
            base_get_log_handler,
            base_set_log_handler
        ));
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(128)))]
    #[test]
    fn random_frames_match_original_c(format in 1i32..=25, width in 1u32..=127,
        height in 1u32..=97, seed in any::<u8>()) {
        check(format, width, height, seed);
    }
}
