//! Original-C parity for the video level queries (#5187): the thin ABI shims
//! and real field adapter against the extracted C, by exact binary32 bits.
//! Excludes a dead core and concurrent level writes, matching C's contract.
//! No intentional differences.
use core::mem::{align_of, offset_of, size_of};
use obs_core::ffi::video_level_queries::{
    Fields, obs_get_video_hdr_nominal_peak_level, obs_get_video_sdr_white_level,
};
use obs_core::video_level_queries::{VideoLevels, hdr_nominal_peak_level, sdr_white_level};
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_video_level_queries_layout(layout: *mut usize);
    fn oracle_video_level_queries_check(
        mode: i32,
        white: u32,
        peak: u32,
        white_fn: unsafe extern "C" fn() -> f32,
        peak_fn: unsafe extern "C" fn() -> f32,
        out: *mut u32,
    ) -> bool;
}

#[test]
fn real_c_adapter_layout() {
    let mut layout = [0; 5];
    // SAFETY: C writes exactly five layout values into this array.
    unsafe { oracle_video_level_queries_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<Fields>(),
            align_of::<Fields>(),
            offset_of!(Fields, graphics),
            offset_of!(Fields, sdr_white_level),
            offset_of!(Fields, hdr_nominal_peak_level),
        ]
    );
}

fn check(mode: i32, white: u32, peak: u32) {
    let mut out = [0u32; 2];
    // SAFETY: C owns a live real-layout core fixture, installs it as the
    // thread's current core and never dereferences the graphics token.
    assert!(
        unsafe {
            oracle_video_level_queries_check(
                mode,
                white,
                peak,
                obs_get_video_sdr_white_level,
                obs_get_video_hdr_nominal_peak_level,
                out.as_mut_ptr(),
            )
        },
        "mode {mode} white {white:#010x} peak {peak:#010x}"
    );
    // The safe core gives the same bits as the original C.
    let levels = (mode != 0).then_some(VideoLevels {
        graphics: mode == 2,
        sdr_white_level: f32::from_bits(white),
        hdr_nominal_peak_level: f32::from_bits(peak),
    });
    assert_eq!(sdr_white_level(levels).to_bits(), out[0]);
    assert_eq!(hdr_nominal_peak_level(levels).to_bits(), out[1]);
}

const EDGES: [u32; 14] = [
    0x0000_0000, // +0
    0x8000_0000, // -0
    0x0000_0001, // smallest subnormal
    0x0080_0000, // smallest normal
    0x7f7f_ffff, // largest finite
    0x7f80_0000, // +inf
    0xff80_0000, // -inf
    0x7fc0_0000, // quiet NaN
    0x7fc1_2345, // quiet NaN payload
    0xffc5_4321, // negative quiet NaN payload
    0xbf80_0000, // -1
    0x4396_0000, // 300 (the SDR default)
    0x447a_0000, // 1000 (the HDR default)
    0x4323_4567,
];

#[test]
fn characterized_edges_match_original_c() {
    for mode in 0..3 {
        for white in EDGES {
            for peak in EDGES {
                check(mode, white, peak);
            }
        }
    }
}

// Signaling NaNs are excluded: binary32 returns through x87 on 32-bit x86
// may quiet them in both implementations alike.
fn not_signaling_nan(bits: u32) -> bool {
    let nan = (bits & 0x7f80_0000) == 0x7f80_0000 && (bits & 0x007f_ffff) != 0;
    !nan || bits & 0x0040_0000 != 0
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn random_level_bits_match_original_c(
        mode in 0..3i32,
        white in any::<u32>().prop_filter("signaling NaN", |b| not_signaling_nan(*b)),
        peak in any::<u32>().prop_filter("signaling NaN", |b| not_signaling_nan(*b)),
    ) {
        check(mode, white, peak);
    }
}
