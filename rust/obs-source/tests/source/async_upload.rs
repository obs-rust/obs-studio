//! Tier 1: `obs_source::async_upload`, the async video upload decisions.
//!
//! The `steps_*` cases port `test/cmocka/test_async_upload_steps.c` 1:1,
//! under the same case names, in the same order, with the same inputs and
//! expectations, the floats compared as bits.
//!
//! The `edge_*` cases are new: the compare against a model of the five
//! fields, each changed alone and all together, over any values; the size
//! uniforms against models in `f64` (exact for these one-operation results)
//! for every size around each power of two and by proptest over every `u32`;
//! the HLG clamp and the white level ratio at every special `f32` bit
//! pattern (zeros, subnormals, the ends of the normals, infinities, quiet
//! and signalling NaNs of both signs) and by proptest over any bits; every
//! transfer, flag byte and `max_luminance`; the routes for convert types
//! around the enum and at the ends of `i32`; every BGR swap pair of formats
//! from -64 to 64 and the ends of `i32`.

use obs_source::async_upload::{
    CONVERT_NONE, FRAME_LINEAR_ALPHA, Format, bgr_swap, gs_color_format, hdr_lw, hlg_peak,
    linear_alpha, maximum_nits, over_sdr_white, plain, reanchor, reanchor_reads_unbuffered,
    sets_range, size_uniforms, texture_changed, tries_gpu, uses_texrender, video_format, video_trc,
};
use proptest::prelude::*;

const F_NAN: u32 = 0x7fc0_0000;
const F_INF: u32 = 0x7f80_0000;
const F_NEG_INF: u32 = 0xff80_0000;

/// `OBS_SOURCE_CONVERT_R10L`, the last convert type.
const CONVERT_R10L: i32 = 20;

fn bits(f: f32) -> u32 {
    f.to_bits()
}

fn float(u: u32) -> f32 {
    f32::from_bits(u)
}

fn fmt(width: u32, height: u32, format: i32, full_range: bool, trc: u8) -> Format {
    Format {
        width,
        height,
        format,
        full_range,
        trc,
    }
}

// --- enum values and layouts ---------------------------------------------

#[test]
fn steps_enum_values_and_layouts() {
    // The struct layouts are the shims' (Tier 3 checks them against the C);
    // the values are the ones the core compares with.
    assert_eq!(gs_color_format::UNKNOWN, 0);
    assert_eq!(gs_color_format::BGRX, 4);
    assert_eq!(gs_color_format::BGRA, 5);
    assert_eq!(gs_color_format::BGRA_UNORM, 21);
    assert_eq!(video_format::NONE, 0);
    assert_eq!(video_format::RGBA, 6);
    assert_eq!(video_format::BGRA, 7);
    assert_eq!(video_format::BGRX, 8);
    assert_eq!(video_format::R10L, 25);
    assert_eq!(video_trc::DEFAULT, 0);
    assert_eq!(video_trc::SRGB, 1);
    assert_eq!(video_trc::PQ, 2);
    assert_eq!(video_trc::HLG, 3);
    assert_eq!(FRAME_LINEAR_ALPHA, 1);
    assert_eq!(CONVERT_NONE, 0);
}

// --- set_async_texture_size() --------------------------------------------

#[test]
fn steps_texture_changed_each_field() {
    let base = fmt(1920, 1080, 2, false, video_trc::SRGB); // VIDEO_FORMAT_NV12

    assert!(!texture_changed(base, base));

    let changed = [
        Format {
            width: 1919,
            ..base
        },
        Format {
            height: 1081,
            ..base
        },
        Format { format: 1, ..base }, // VIDEO_FORMAT_I420
        Format {
            full_range: true,
            ..base
        },
        Format {
            trc: video_trc::PQ,
            ..base
        },
    ];
    for f in changed {
        assert!(texture_changed(base, f), "{f:?}");
        assert!(texture_changed(f, base), "{f:?}");
    }

    // all five at once
    let all = fmt(1280, 720, video_format::BGRA, true, video_trc::HLG);
    assert!(texture_changed(base, all));
}

#[test]
fn steps_texture_changed_first_frame() {
    // characterized, not endorsed: a 0 x 0 frame of the zero values keeps
    // the (absent) textures
    let zero = fmt(0, 0, video_format::NONE, false, video_trc::DEFAULT);
    assert!(!texture_changed(zero, zero));
    assert!(texture_changed(
        zero,
        fmt(1, 0, video_format::NONE, false, 0)
    ));
    assert!(texture_changed(
        zero,
        fmt(0, 1, video_format::NONE, false, 0)
    ));
    assert!(texture_changed(zero, fmt(0, 0, 1, false, 0)));
    assert!(texture_changed(
        zero,
        fmt(0, 0, video_format::NONE, true, 0)
    ));
    assert!(texture_changed(
        zero,
        fmt(0, 0, video_format::NONE, false, 1)
    ));
}

#[test]
fn steps_texture_changed_edges() {
    let big = fmt(u32::MAX, u32::MAX, i32::MAX, true, 255);
    assert!(!texture_changed(big, big));
    assert!(texture_changed(
        big,
        fmt(u32::MAX - 1, u32::MAX, i32::MAX, true, 255)
    ));
    assert!(texture_changed(big, fmt(u32::MAX, 0, i32::MAX, true, 255)));
    assert!(texture_changed(
        big,
        fmt(u32::MAX, u32::MAX, i32::MIN, true, 255)
    ));
    assert!(texture_changed(
        big,
        fmt(u32::MAX, u32::MAX, i32::MAX, true, 254)
    ));
    assert!(!texture_changed(
        fmt(1, 1, -1, false, 4),
        fmt(1, 1, -1, false, 4)
    ));
    assert!(texture_changed(
        fmt(1, 1, -1, false, 4),
        fmt(1, 1, 26, false, 4)
    ));
}

#[test]
fn steps_convert_type_routes() {
    assert!(!tries_gpu(CONVERT_NONE));
    assert!(plain(CONVERT_NONE));
    for convert_type in 1..=CONVERT_R10L {
        assert!(tries_gpu(convert_type), "{convert_type}");
        assert!(!plain(convert_type), "{convert_type}");
    }
    for outside in [-1, 21, i32::MIN, i32::MAX] {
        assert!(tries_gpu(outside), "{outside}");
        assert!(!plain(outside), "{outside}");
    }
}

// --- update_async_texrender() --------------------------------------------

/// `(cx, cy, width, height, width_d2, height_d2, width_x2_i, height_x2_i)`.
type SizeCase = (u32, u32, u32, u32, u32, u32, u32, u32);

const SIZES: [SizeCase; 8] = [
    (
        0,
        4_294_967_295,
        0x0000_0000,
        0x4f80_0000,
        0x0000_0000,
        0x4f00_0000,
        F_INF,
        0x2f00_0000,
    ),
    (
        1,
        16_777_217,
        0x3f80_0000,
        0x4b80_0000,
        0x3f00_0000,
        0x4b00_0000,
        0x3f00_0000,
        0x3300_0000,
    ),
    (
        2,
        1920,
        0x4000_0000,
        0x44f0_0000,
        0x3f80_0000,
        0x4470_0000,
        0x3e80_0000,
        0x3988_8889,
    ),
    (
        3,
        1919,
        0x4040_0000,
        0x44ef_e000,
        0x3fc0_0000,
        0x446f_e000,
        0x3e2a_aaab,
        0x3988_9abf,
    ),
    (
        1919,
        3,
        0x44ef_e000,
        0x4040_0000,
        0x446f_e000,
        0x3fc0_0000,
        0x3988_9abf,
        0x3e2a_aaab,
    ),
    (
        1920,
        2,
        0x44f0_0000,
        0x4000_0000,
        0x4470_0000,
        0x3f80_0000,
        0x3988_8889,
        0x3e80_0000,
    ),
    (
        16_777_217,
        1,
        0x4b80_0000,
        0x3f80_0000,
        0x4b00_0000,
        0x3f00_0000,
        0x3300_0000,
        0x3f00_0000,
    ),
    (
        4_294_967_295,
        0,
        0x4f80_0000,
        0x0000_0000,
        0x4f00_0000,
        0x0000_0000,
        0x2f00_0000,
        F_INF,
    ),
];

#[test]
fn steps_size_uniforms() {
    for (cx, cy, w, h, wd2, hd2, wx2i, hx2i) in SIZES {
        let s = size_uniforms(cx, cy);
        assert_eq!(bits(s.width), w, "{cx}");
        assert_eq!(bits(s.height), h, "{cy}");
        assert_eq!(bits(s.width_d2), wd2, "{cx}");
        assert_eq!(bits(s.height_d2), hd2, "{cy}");
        assert_eq!(bits(s.width_x2_i), wx2i, "{cx}");
        assert_eq!(bits(s.height_x2_i), hx2i, "{cy}");
    }
}

#[test]
fn steps_hlg_peak() {
    let cases: [(u32, u32); 14] = [
        (bits(100.0), 0x42c8_0000),
        (bits(999.0), 0x4479_c000),
        (bits(1000.0), 0x447a_0000),
        (0x447a_0001, 0x447a_0000),
        (bits(1001.0), 0x447a_0000),
        (bits(10000.0), 0x447a_0000),
        (F_INF, 0x447a_0000),
        (F_NAN, F_NAN),
        (0xffc0_0001, 0xffc0_0001),
        (0x0000_0000, 0x0000_0000),
        (0x8000_0000, 0x8000_0000),
        (0x0000_0001, 0x0000_0001),
        (bits(-5.0), 0xc0a0_0000),
        (F_NEG_INF, F_NEG_INF),
    ];
    for (input, want) in cases {
        assert_eq!(bits(hlg_peak(float(input))), want, "{input:#010x}");
    }
}

#[test]
fn steps_maximum_nits() {
    assert_eq!(bits(maximum_nits(video_trc::HLG, 1000.0)), 0x447a_0000);
    assert_eq!(bits(maximum_nits(video_trc::HLG, 100.0)), 0x42c8_0000);
    assert_eq!(bits(maximum_nits(video_trc::HLG, float(F_NAN))), F_NAN);
    for trc in [video_trc::DEFAULT, video_trc::SRGB, video_trc::PQ, 4, 255] {
        assert_eq!(bits(maximum_nits(trc, 1000.0)), 0x461c_4000, "{trc}");
        assert_eq!(bits(maximum_nits(trc, float(F_NAN))), 0x461c_4000, "{trc}");
    }
}

/// `(trc, peak, white, hlg_peak, maximum_nits, over_white)`.
type LevelCase = (u8, f32, f32, u32, u32, u32);

const LEVELS: [LevelCase; 48] = [
    (1, 100.0, 80.0, 0x42c8_0000, 0x461c_4000, 0x42fa_0000),
    (1, 100.0, 300.0, 0x42c8_0000, 0x461c_4000, 0x4205_5555),
    (1, 100.0, 480.0, 0x42c8_0000, 0x461c_4000, 0x41a6_aaab),
    (1, 1000.0, 80.0, 0x447a_0000, 0x461c_4000, 0x42fa_0000),
    (1, 1000.0, 300.0, 0x447a_0000, 0x461c_4000, 0x4205_5555),
    (1, 1000.0, 480.0, 0x447a_0000, 0x461c_4000, 0x41a6_aaab),
    (1, 1001.0, 80.0, 0x447a_0000, 0x461c_4000, 0x42fa_0000),
    (1, 1001.0, 300.0, 0x447a_0000, 0x461c_4000, 0x4205_5555),
    (1, 1001.0, 480.0, 0x447a_0000, 0x461c_4000, 0x41a6_aaab),
    (1, 10000.0, 80.0, 0x447a_0000, 0x461c_4000, 0x42fa_0000),
    (1, 10000.0, 300.0, 0x447a_0000, 0x461c_4000, 0x4205_5555),
    (1, 10000.0, 480.0, 0x447a_0000, 0x461c_4000, 0x41a6_aaab),
    (2, 100.0, 80.0, 0x42c8_0000, 0x461c_4000, 0x42fa_0000),
    (2, 100.0, 300.0, 0x42c8_0000, 0x461c_4000, 0x4205_5555),
    (2, 100.0, 480.0, 0x42c8_0000, 0x461c_4000, 0x41a6_aaab),
    (2, 1000.0, 80.0, 0x447a_0000, 0x461c_4000, 0x42fa_0000),
    (2, 1000.0, 300.0, 0x447a_0000, 0x461c_4000, 0x4205_5555),
    (2, 1000.0, 480.0, 0x447a_0000, 0x461c_4000, 0x41a6_aaab),
    (2, 1001.0, 80.0, 0x447a_0000, 0x461c_4000, 0x42fa_0000),
    (2, 1001.0, 300.0, 0x447a_0000, 0x461c_4000, 0x4205_5555),
    (2, 1001.0, 480.0, 0x447a_0000, 0x461c_4000, 0x41a6_aaab),
    (2, 10000.0, 80.0, 0x447a_0000, 0x461c_4000, 0x42fa_0000),
    (2, 10000.0, 300.0, 0x447a_0000, 0x461c_4000, 0x4205_5555),
    (2, 10000.0, 480.0, 0x447a_0000, 0x461c_4000, 0x41a6_aaab),
    (3, 100.0, 80.0, 0x42c8_0000, 0x42c8_0000, 0x3fa0_0000),
    (3, 100.0, 300.0, 0x42c8_0000, 0x42c8_0000, 0x3eaa_aaab),
    (3, 100.0, 480.0, 0x42c8_0000, 0x42c8_0000, 0x3e55_5555),
    (3, 1000.0, 80.0, 0x447a_0000, 0x447a_0000, 0x4148_0000),
    (3, 1000.0, 300.0, 0x447a_0000, 0x447a_0000, 0x4055_5555),
    (3, 1000.0, 480.0, 0x447a_0000, 0x447a_0000, 0x4005_5555),
    (3, 1001.0, 80.0, 0x447a_0000, 0x447a_0000, 0x4148_0000),
    (3, 1001.0, 300.0, 0x447a_0000, 0x447a_0000, 0x4055_5555),
    (3, 1001.0, 480.0, 0x447a_0000, 0x447a_0000, 0x4005_5555),
    (3, 10000.0, 80.0, 0x447a_0000, 0x447a_0000, 0x4148_0000),
    (3, 10000.0, 300.0, 0x447a_0000, 0x447a_0000, 0x4055_5555),
    (3, 10000.0, 480.0, 0x447a_0000, 0x447a_0000, 0x4005_5555),
    (4, 100.0, 80.0, 0x42c8_0000, 0x461c_4000, 0x42fa_0000),
    (4, 100.0, 300.0, 0x42c8_0000, 0x461c_4000, 0x4205_5555),
    (4, 100.0, 480.0, 0x42c8_0000, 0x461c_4000, 0x41a6_aaab),
    (4, 1000.0, 80.0, 0x447a_0000, 0x461c_4000, 0x42fa_0000),
    (4, 1000.0, 300.0, 0x447a_0000, 0x461c_4000, 0x4205_5555),
    (4, 1000.0, 480.0, 0x447a_0000, 0x461c_4000, 0x41a6_aaab),
    (4, 1001.0, 80.0, 0x447a_0000, 0x461c_4000, 0x42fa_0000),
    (4, 1001.0, 300.0, 0x447a_0000, 0x461c_4000, 0x4205_5555),
    (4, 1001.0, 480.0, 0x447a_0000, 0x461c_4000, 0x41a6_aaab),
    (4, 10000.0, 80.0, 0x447a_0000, 0x461c_4000, 0x42fa_0000),
    (4, 10000.0, 300.0, 0x447a_0000, 0x461c_4000, 0x4205_5555),
    (4, 10000.0, 480.0, 0x447a_0000, 0x461c_4000, 0x41a6_aaab),
];

#[test]
fn steps_level_uniforms() {
    for (trc, peak, white, want_peak, want_nits, want_over) in LEVELS {
        let p = hlg_peak(peak);
        let nits = maximum_nits(trc, p);
        let over = over_sdr_white(nits, white);
        assert_eq!(bits(p), want_peak, "{trc} {peak} {white}");
        assert_eq!(bits(nits), want_nits, "{trc} {peak} {white}");
        assert_eq!(bits(over), want_over, "{trc} {peak} {white}");
    }
}

#[test]
fn steps_over_sdr_white_edges() {
    assert_eq!(bits(over_sdr_white(10000.0, 0.0)), F_INF);
    assert_eq!(bits(over_sdr_white(10000.0, -0.0)), F_NEG_INF);
    assert_eq!(bits(over_sdr_white(10000.0, float(F_INF))), 0);
    assert!(over_sdr_white(0.0, 0.0).is_nan());
    assert_eq!(bits(over_sdr_white(float(F_NAN), 300.0)), F_NAN);
    assert_eq!(bits(over_sdr_white(10000.0, float(F_NAN))), F_NAN);
    // the smallest subnormal white level overflows
    assert_eq!(bits(over_sdr_white(10000.0, float(1))), F_INF);
    // rounded once, as the C's float division: 1000 / 3
    assert_eq!(bits(over_sdr_white(1000.0, 3.0)), 0x43a6_aaab);
}

#[test]
fn steps_hdr_lw() {
    assert_eq!(bits(hdr_lw(0)), 0);
    assert_eq!(bits(hdr_lw(1)), 0x3f80_0000);
    assert_eq!(bits(hdr_lw(1000)), 0x447a_0000);
    assert_eq!(bits(hdr_lw(10000)), 0x461c_4000);
    assert_eq!(bits(hdr_lw(65535)), 0x477f_ff00);
    for v in 0..=u16::MAX {
        assert_eq!(bits(hdr_lw(v)), bits(f32::from(v)), "{v}");
    }
}

#[test]
fn steps_sets_range() {
    assert!(sets_range(false));
    assert!(!sets_range(true));
}

// --- update_async_textures() ---------------------------------------------

#[test]
fn steps_linear_alpha() {
    for flags in 0..=u8::MAX {
        assert_eq!(linear_alpha(flags), flags & 1 != 0, "{flags}");
    }
}

#[test]
fn steps_uses_texrender() {
    assert!(!uses_texrender(false, false));
    assert!(!uses_texrender(false, true));
    assert!(!uses_texrender(true, false));
    assert!(uses_texrender(true, true));
}

// --- check_to_swap_bgrx_bgra() --------------------------------------------

/// `GS_RG16`, the last `enum gs_color_format` value.
const GS_RG16: i32 = 22;

fn model_swap(tex: i32, frame: i32) -> Option<i32> {
    match (tex, frame) {
        (4, 7) => Some(5),
        (5, 8) => Some(4),
        _ => None,
    }
}

#[test]
fn steps_bgr_swap_every_pair() {
    let mut swaps = 0;
    for tex in -1..=GS_RG16 + 1 {
        for frame in -1..=video_format::R10L + 1 {
            let want = model_swap(tex, frame);
            assert_eq!(bgr_swap(tex, frame), want, "{tex} {frame}");
            swaps += usize::from(want.is_some());
        }
    }
    assert_eq!(swaps, 2);
    assert_eq!(bgr_swap(gs_color_format::BGRX, video_format::BGRX), None);
    assert_eq!(bgr_swap(gs_color_format::BGRA, video_format::BGRA), None);
    assert_eq!(bgr_swap(20, video_format::BGRA), None); // GS_BGRX_UNORM
    assert_eq!(
        bgr_swap(gs_color_format::BGRA_UNORM, video_format::BGRX),
        None
    );
    assert_eq!(bgr_swap(gs_color_format::RGBA, video_format::RGBA), None);
    assert_eq!(bgr_swap(i32::MIN, i32::MAX), None);
}

// --- obs_source_update_async_video() ---------------------------------------

#[test]
fn steps_reanchor() {
    assert!(!reanchor_reads_unbuffered(false));
    assert!(reanchor_reads_unbuffered(true));
    assert!(reanchor(false, false));
    assert!(reanchor(false, true));
    assert!(reanchor(true, false));
    assert!(!reanchor(true, true));
}

// --- edge cases --------------------------------------------------------------

/// Every special `f32` bit pattern: zeros, the smallest and largest
/// subnormals, the smallest and largest normals, one, the clamp and the
/// levels around it, infinities, quiet and signalling NaNs of both signs.
const SPECIAL: [u32; 26] = [
    0x0000_0000,
    0x8000_0000,
    0x0000_0001,
    0x8000_0001,
    0x007f_ffff,
    0x807f_ffff,
    0x0080_0000,
    0x8080_0000,
    0x7f7f_ffff,
    0xff7f_ffff,
    0x3f80_0000,
    0xbf80_0000,
    0x447a_0000, // 1000
    0x4479_ffff, // just below 1000
    0x447a_0001, // just above 1000
    0x42a0_0000, // 80
    0x4396_0000, // 300
    0x43f0_0000, // 480
    0x461c_4000, // 10000
    F_INF,
    F_NEG_INF,
    F_NAN,
    0xffc0_0000,
    0x7fc0_0001,
    0x7fa0_0000, // signalling
    0xff80_0001, // signalling, negative
];

/// The C's `>` against 1000: true only for a number above it.
fn model_hlg_peak(level: f32) -> f32 {
    match level.partial_cmp(&1000.0) {
        Some(core::cmp::Ordering::Greater) => 1000.0,
        _ => level,
    }
}

#[test]
fn edge_hlg_peak_special_bits() {
    for b in SPECIAL {
        assert_eq!(
            bits(hlg_peak(float(b))),
            bits(model_hlg_peak(float(b))),
            "{b:#010x}"
        );
    }
    // every float from just below 1000 to just above it, by bits
    for b in 0x4479_ff00..=0x447a_0100 {
        let want = if b > 0x447a_0000 { 0x447a_0000 } else { b };
        assert_eq!(bits(hlg_peak(float(b))), want, "{b:#010x}");
    }
}

/// The ratio in `f64`: exact for one `f32` division of finite numbers
/// (53 bits hold the product of two 24-bit significands, so rounding the
/// `f64` quotient to `f32` rounds once). NaNs compare as NaN.
#[allow(clippy::cast_possible_truncation)]
fn model_over(nits: f32, white: f32) -> f32 {
    (f64::from(nits) / f64::from(white)) as f32
}

fn same_or_both_nan(a: f32, b: f32) -> bool {
    bits(a) == bits(b) || (a.is_nan() && b.is_nan())
}

#[test]
fn edge_over_sdr_white_special_bits() {
    for n in SPECIAL {
        for w in SPECIAL {
            let got = over_sdr_white(float(n), float(w));
            let want = model_over(float(n), float(w));
            assert!(
                same_or_both_nan(got, want),
                "{n:#010x} / {w:#010x}: {got:e} vs {want:e}"
            );
        }
    }
}

#[test]
fn edge_maximum_nits_every_transfer() {
    for trc in 0..=u8::MAX {
        for p in SPECIAL {
            let got = bits(maximum_nits(trc, float(p)));
            let want = if trc == 3 { p } else { 0x461c_4000 };
            assert_eq!(got, want, "{trc} {p:#010x}");
        }
    }
}

/// `(float)x` through `f64`: exact, then rounded once to nearest even.
#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
fn model_size(x: u32) -> SizeModel {
    let w = f64::from(x) as f32;
    SizeModel {
        width: bits(w),
        half: bits((f64::from(w) * 0.5) as f32),
        half_texel: bits((0.5 / f64::from(w)) as f32),
    }
}

#[derive(Debug, PartialEq, Eq)]
struct SizeModel {
    width: u32,
    half: u32,
    half_texel: u32,
}

fn check_size(cx: u32, cy: u32) {
    let s = size_uniforms(cx, cy);
    let (mx, my) = (model_size(cx), model_size(cy));
    assert_eq!(bits(s.width), mx.width, "{cx}");
    assert_eq!(bits(s.width_d2), mx.half, "{cx}");
    assert_eq!(bits(s.width_x2_i), mx.half_texel, "{cx}");
    assert_eq!(bits(s.height), my.width, "{cy}");
    assert_eq!(bits(s.height_d2), my.half, "{cy}");
    assert_eq!(bits(s.height_x2_i), my.half_texel, "{cy}");
}

#[test]
fn edge_size_uniforms_around_powers_of_two() {
    for shift in 0..32 {
        let p = 1u32 << shift;
        for d in 0..=4u32 {
            let lo = p.wrapping_sub(d);
            let hi = p.wrapping_add(d);
            check_size(lo, hi);
            check_size(hi, lo);
        }
    }
    for x in [0, 1, 1919, 1920, 3840, 7680, u32::MAX - 1, u32::MAX] {
        check_size(x, x);
    }
}

#[test]
fn edge_size_uniforms_axes_independent() {
    // each axis reads only its own size
    for (a, b) in [(1, 2), (1920, 1080), (0, u32::MAX), (16_777_217, 3)] {
        let ab = size_uniforms(a, b);
        let ba = size_uniforms(b, a);
        assert_eq!(bits(ab.width), bits(ba.height));
        assert_eq!(bits(ab.width_d2), bits(ba.height_d2));
        assert_eq!(bits(ab.width_x2_i), bits(ba.height_x2_i));
    }
}

#[test]
fn edge_routes_around_enum() {
    for convert_type in (-1000..=1000).chain([i32::MIN, i32::MIN + 1, i32::MAX - 1, i32::MAX]) {
        assert_eq!(tries_gpu(convert_type), convert_type != 0, "{convert_type}");
        assert_eq!(plain(convert_type), convert_type == 0, "{convert_type}");
        assert_ne!(tries_gpu(convert_type), plain(convert_type));
    }
}

#[test]
fn edge_bgr_swap_wide() {
    let ends = [i32::MIN, i32::MIN + 1, i32::MAX - 1, i32::MAX];
    for tex in (-64..=64).chain(ends) {
        for frame in (-64..=64).chain(ends) {
            assert_eq!(
                bgr_swap(tex, frame),
                model_swap(tex, frame),
                "{tex} {frame}"
            );
        }
    }
}

#[test]
fn edge_texture_changed_each_field_alone() {
    let bases = [
        Format::default(),
        fmt(1920, 1080, 2, false, 1),
        fmt(u32::MAX, u32::MAX, i32::MAX, true, 255),
        fmt(1, 0, i32::MIN, true, 0),
    ];
    for b in bases {
        assert!(!texture_changed(b, b));
        let alone = [
            Format {
                width: b.width ^ 1,
                ..b
            },
            Format {
                width: b.width.wrapping_add(1 << 31),
                ..b
            },
            Format {
                height: b.height ^ 1,
                ..b
            },
            Format {
                format: b.format ^ 1,
                ..b
            },
            Format {
                format: b.format ^ i32::MIN,
                ..b
            },
            Format {
                full_range: !b.full_range,
                ..b
            },
            Format {
                trc: b.trc ^ 1,
                ..b
            },
            Format {
                trc: b.trc ^ 0x80,
                ..b
            },
        ];
        for f in alone {
            assert!(texture_changed(b, f), "{b:?} {f:?}");
            assert!(texture_changed(f, b), "{f:?} {b:?}");
        }
    }
}

fn any_format() -> impl Strategy<Value = Format> {
    (
        prop_oneof![Just(0u32), Just(1920), Just(u32::MAX), any::<u32>()],
        prop_oneof![Just(0u32), Just(1080), Just(u32::MAX), any::<u32>()],
        prop_oneof![-1..27i32, any::<i32>()],
        any::<bool>(),
        prop_oneof![0..5u8, any::<u8>()],
    )
        .prop_map(|(width, height, format, full_range, trc)| Format {
            width,
            height,
            format,
            full_range,
            trc,
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20000))]

    #[test]
    fn edge_prop_texture_changed(cur in any_format(), frame in any_format(), same in any::<bool>()) {
        let frame = if same { cur } else { frame };
        let want = cur.width != frame.width
            || cur.height != frame.height
            || cur.format != frame.format
            || cur.full_range != frame.full_range
            || cur.trc != frame.trc;
        prop_assert_eq!(texture_changed(cur, frame), want);
        prop_assert_eq!(texture_changed(frame, cur), want);
    }

    #[test]
    fn edge_prop_size_uniforms(cx in any::<u32>(), cy in any::<u32>()) {
        check_size(cx, cy);
    }

    #[test]
    fn edge_prop_levels(peak in any::<u32>(), white in any::<u32>(), trc in any::<u8>()) {
        let p = hlg_peak(float(peak));
        prop_assert_eq!(bits(p), bits(model_hlg_peak(float(peak))));
        let nits = maximum_nits(trc, p);
        prop_assert_eq!(bits(nits), if trc == 3 { bits(p) } else { 0x461c_4000 });
        let over = over_sdr_white(nits, float(white));
        prop_assert!(same_or_both_nan(over, model_over(nits, float(white))));
    }

    #[test]
    fn edge_prop_flags_and_routes(flags in any::<u8>(), gpu in any::<bool>(), has in any::<bool>(),
                                  decoupled in any::<bool>(), unbuffered in any::<bool>()) {
        prop_assert_eq!(linear_alpha(flags), flags % 2 == 1);
        prop_assert_eq!(uses_texrender(gpu, has), gpu && has);
        prop_assert_eq!(reanchor_reads_unbuffered(decoupled), decoupled);
        prop_assert_eq!(reanchor(decoupled, unbuffered), !(decoupled && unbuffered));
    }
}
