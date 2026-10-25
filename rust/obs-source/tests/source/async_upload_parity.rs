//! Tier 3: the `obs_source_async_upload_*` shims against the original C
//! (`obs-c-oracle`, `oracle/source_async_upload.c`: the unmodified
//! `libobs/obs-source-async-upload.c.h` with its functions renamed).
//!
//! Every function takes and returns plain values, so a run compares the
//! bytes of the two results: the C's `bool` as the byte it returns, the
//! format as its value, every `float` as its bits (NaN payloads and signs
//! included).
//!
//! Runs:
//! - the two structs' sizes, alignments and offsets, the enum values and
//!   the type sizes against the C headers;
//! - the compare with each field changed alone and all at once, from bases
//!   at the ends of every field;
//! - the size uniforms for every size from 0 to 70,000, around every power
//!   of two, every 65,521st `u32` and the ends;
//! - the HLG clamp for every 4,099th `f32` bit pattern, every pattern within
//!   4,096 of 1000 and every special pattern; the maximum nits for every
//!   transfer at each special pattern; the white level ratio for every pair
//!   of special patterns;
//! - `hdr_lw` for every `uint16_t`, the linear alpha flag for every flag
//!   byte, the routes for every convert type from -100,000 to 100,000 and
//!   the ends, every BGR swap pair from -300 to 300 and the ends, every
//!   re-anchor and texrender case;
//! - the uniforms of `update_async_texrender()` in its order (the size
//!   uniforms, the HLG peak, the maximum nits, the ratio, `hdr_lw`, the
//!   range), one copy on each side, exhaustive over every transfer, both
//!   ranges and the special levels;
//! - proptests (20,000 cases each): every function over any input, the
//!   levels as any bit pattern or drawn from finite, infinite, NaN and
//!   subnormal patterns.
//!
//! No input reaches an integer division or a 128-bit multiply on either
//! side (the only divisions are `f32`, which cannot trap), so no input is
//! skipped on MSVC x64. No intentional differences: there is no pointer to
//! check.

use std::mem::{align_of, offset_of, size_of};
use std::os::raw::c_int;

use obs_c_oracle::source_async_upload::{self as oracle, OracleUploadFormat};
use obs_source::async_upload::{
    CONVERT_NONE, FRAME_LINEAR_ALPHA, gs_color_format, video_format, video_trc,
};
use obs_source::ffi::source_async_upload::{
    self as shim, obs_source_async_upload_format as ShimFormat,
    obs_source_async_upload_size as ShimSize,
};
use proptest::prelude::*;

#[test]
fn values_match_the_c_headers() {
    let rust: [i64; 27] = [
        size_of::<ShimFormat>().try_into().unwrap(),
        align_of::<ShimFormat>().try_into().unwrap(),
        offset_of!(ShimFormat, width).try_into().unwrap(),
        offset_of!(ShimFormat, height).try_into().unwrap(),
        offset_of!(ShimFormat, format).try_into().unwrap(),
        offset_of!(ShimFormat, full_range).try_into().unwrap(),
        offset_of!(ShimFormat, trc).try_into().unwrap(),
        size_of::<ShimSize>().try_into().unwrap(),
        align_of::<ShimSize>().try_into().unwrap(),
        offset_of!(ShimSize, width).try_into().unwrap(),
        offset_of!(ShimSize, height).try_into().unwrap(),
        offset_of!(ShimSize, width_d2).try_into().unwrap(),
        offset_of!(ShimSize, height_d2).try_into().unwrap(),
        offset_of!(ShimSize, width_x2_i).try_into().unwrap(),
        offset_of!(ShimSize, height_x2_i).try_into().unwrap(),
        gs_color_format::UNKNOWN.into(),
        gs_color_format::BGRX.into(),
        gs_color_format::BGRA.into(),
        video_format::BGRA.into(),
        video_format::BGRX.into(),
        video_trc::HLG.into(),
        FRAME_LINEAR_ALPHA.into(),
        CONVERT_NONE.into(),
        size_of::<c_int>().try_into().unwrap(),
        size_of::<c_int>().try_into().unwrap(),
        size_of::<bool>().try_into().unwrap(),
        size_of::<f32>().try_into().unwrap(),
    ];
    for (which, (name, value)) in oracle::VALUE_NAMES.iter().zip(rust).enumerate() {
        // SAFETY: the oracle's table lookup reads only its own constants.
        let c = unsafe { oracle::oracle_source_async_upload_value(which) };
        assert_eq!(c, value, "{name}");
    }
    // SAFETY: as above; past the end it returns INT64_MIN.
    let past = unsafe { oracle::oracle_source_async_upload_value(oracle::VALUE_NAMES.len()) };
    assert_eq!(past, i64::MIN);
    // the two mirrors are the same layout
    assert_eq!(size_of::<OracleUploadFormat>(), size_of::<ShimFormat>());
    assert_eq!(size_of::<oracle::OracleUploadSize>(), size_of::<ShimSize>());
}

// --- one call on each side ----------------------------------------------------

fn c_fmt(f: ShimFormat) -> OracleUploadFormat {
    OracleUploadFormat {
        width: f.width,
        height: f.height,
        format: f.format,
        full_range: f.full_range,
        trc: f.trc,
    }
}

fn check_changed(cur: ShimFormat, frame: ShimFormat) {
    // SAFETY: plain values in and out.
    let c =
        unsafe { oracle::oracle_obs_source_async_upload_texture_changed(c_fmt(cur), c_fmt(frame)) };
    let r = shim::obs_source_async_upload_texture_changed(cur, frame);
    assert_eq!(c, u8::from(r), "{cur:?} {frame:?}");
}

fn size_bits(s: [f32; 6]) -> [u32; 6] {
    s.map(f32::to_bits)
}

fn check_size(cx: u32, cy: u32) {
    // SAFETY: plain values in and out.
    let c = unsafe { oracle::oracle_obs_source_async_upload_size_uniforms(cx, cy) };
    let r = shim::obs_source_async_upload_size_uniforms(cx, cy);
    assert_eq!(
        size_bits([
            c.width,
            c.height,
            c.width_d2,
            c.height_d2,
            c.width_x2_i,
            c.height_x2_i
        ]),
        size_bits([
            r.width,
            r.height,
            r.width_d2,
            r.height_d2,
            r.width_x2_i,
            r.height_x2_i
        ]),
        "{cx} x {cy}"
    );
}

fn check_hlg_peak(b: u32) {
    let f = f32::from_bits(b);
    // SAFETY: plain values in and out.
    let c = unsafe { oracle::oracle_obs_source_async_upload_hlg_peak(f) };
    assert_eq!(
        c.to_bits(),
        shim::obs_source_async_upload_hlg_peak(f).to_bits(),
        "{b:#010x}"
    );
}

fn check_maximum_nits(trc: u8, b: u32) {
    let f = f32::from_bits(b);
    // SAFETY: plain values in and out.
    let c = unsafe { oracle::oracle_obs_source_async_upload_maximum_nits(trc, f) };
    let r = shim::obs_source_async_upload_maximum_nits(trc, f);
    assert_eq!(c.to_bits(), r.to_bits(), "{trc} {b:#010x}");
}

fn check_over(n: u32, w: u32) {
    let (nf, wf) = (f32::from_bits(n), f32::from_bits(w));
    // SAFETY: plain values in and out.
    let c = unsafe { oracle::oracle_obs_source_async_upload_over_sdr_white(nf, wf) };
    let r = shim::obs_source_async_upload_over_sdr_white(nf, wf);
    assert_eq!(c.to_bits(), r.to_bits(), "{n:#010x} / {w:#010x}");
}

fn check_routes(convert_type: c_int) {
    // SAFETY: plain values in and out.
    let (gpu, plain) = unsafe {
        (
            oracle::oracle_obs_source_async_upload_tries_gpu(convert_type),
            oracle::oracle_obs_source_async_upload_plain(convert_type),
        )
    };
    assert_eq!(
        gpu,
        u8::from(shim::obs_source_async_upload_tries_gpu(convert_type)),
        "{convert_type}"
    );
    assert_eq!(
        plain,
        u8::from(shim::obs_source_async_upload_plain(convert_type)),
        "{convert_type}"
    );
}

fn check_swap(tex: c_int, frame: c_int) {
    // SAFETY: plain values in and out.
    let c = unsafe { oracle::oracle_obs_source_async_upload_bgr_swap(tex, frame) };
    assert_eq!(
        c,
        shim::obs_source_async_upload_bgr_swap(tex, frame),
        "{tex} {frame}"
    );
}

fn check_flags(
    flags: u8,
    gpu: bool,
    has: bool,
    decoupled: bool,
    unbuffered: bool,
    full_range: bool,
) {
    // SAFETY: plain values in and out.
    let c = unsafe {
        [
            oracle::oracle_obs_source_async_upload_linear_alpha(flags),
            oracle::oracle_obs_source_async_upload_uses_texrender(gpu, has),
            oracle::oracle_obs_source_async_upload_reanchor_reads_unbuffered(decoupled),
            oracle::oracle_obs_source_async_upload_reanchor(decoupled, unbuffered),
            oracle::oracle_obs_source_async_upload_sets_range(full_range),
        ]
    };
    let r = [
        shim::obs_source_async_upload_linear_alpha(flags),
        shim::obs_source_async_upload_uses_texrender(gpu, has),
        shim::obs_source_async_upload_reanchor_reads_unbuffered(decoupled),
        shim::obs_source_async_upload_reanchor(decoupled, unbuffered),
        shim::obs_source_async_upload_sets_range(full_range),
    ]
    .map(u8::from);
    assert_eq!(
        c, r,
        "{flags} {gpu} {has} {decoupled} {unbuffered} {full_range}"
    );
}

/// Every special `f32` bit pattern: zeros, subnormals, the ends of the
/// normals, one, the clamp and its neighbours, the white levels, 10000,
/// infinities, quiet and signalling NaNs of both signs and payloads.
const SPECIAL: [u32; 28] = [
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
    0x42c8_0000, // 100
    0x447a_0000, // 1000
    0x4479_ffff,
    0x447a_0001,
    0x447a_4000, // 1001
    0x42a0_0000, // 80
    0x4396_0000, // 300
    0x43f0_0000, // 480
    0x461c_4000, // 10000
    0x7f80_0000,
    0xff80_0000,
    0x7fc0_0000,
    0xffc0_0000,
    0x7fc0_1234,
    0x7fa0_0000, // signalling
    0xff80_0001, // signalling, negative
];

// --- sweeps ----------------------------------------------------------------------

#[test]
fn texture_changed_each_field() {
    let widths = [0, 1, 1919, 1920, u32::MAX];
    let formats = [i32::MIN, -1, 0, 7, 8, 25, 26, i32::MAX];
    let trcs = [0u8, 1, 2, 3, 4, 255];
    let mut bases = Vec::new();
    for &w in &widths {
        for &f in &formats {
            for &t in &trcs {
                for fr in [false, true] {
                    bases.push(ShimFormat {
                        width: w,
                        height: w.wrapping_add(7),
                        format: f,
                        full_range: fr,
                        trc: t,
                    });
                }
            }
        }
    }
    for b in bases {
        check_changed(b, b);
        let alone = [
            ShimFormat {
                width: b.width.wrapping_add(1),
                ..b
            },
            ShimFormat {
                width: b.width ^ 0x8000_0000,
                ..b
            },
            ShimFormat {
                height: b.height.wrapping_sub(1),
                ..b
            },
            ShimFormat {
                format: b.format.wrapping_add(1),
                ..b
            },
            ShimFormat {
                format: b.format ^ i32::MIN,
                ..b
            },
            ShimFormat {
                full_range: !b.full_range,
                ..b
            },
            ShimFormat {
                trc: b.trc.wrapping_add(1),
                ..b
            },
            ShimFormat {
                width: !b.width,
                height: !b.height,
                format: !b.format,
                full_range: !b.full_range,
                trc: !b.trc,
            },
        ];
        for f in alone {
            check_changed(b, f);
            check_changed(f, b);
        }
    }
}

#[test]
fn size_uniforms_sweep() {
    for x in 0..=70_000u32 {
        check_size(x, 70_000 - x);
    }
    for shift in 0..32 {
        let p = 1u32 << shift;
        for d in 0..=64u32 {
            check_size(p.wrapping_sub(d), p.wrapping_add(d));
            check_size(p.wrapping_add(d), p.wrapping_sub(d));
        }
    }
    let mut x = 0u32;
    loop {
        check_size(x, !x);
        match x.checked_add(65_521) {
            Some(n) => x = n,
            None => break,
        }
    }
    check_size(u32::MAX, u32::MAX);
}

#[test]
fn hlg_peak_sweep() {
    let mut b = 0u32;
    loop {
        check_hlg_peak(b);
        match b.checked_add(4_099) {
            Some(n) => b = n,
            None => break,
        }
    }
    for b in (0x447a_0000 - 4_096)..=(0x447a_0000 + 4_096) {
        check_hlg_peak(b);
    }
    for b in SPECIAL {
        check_hlg_peak(b);
    }
}

#[test]
fn maximum_nits_every_transfer() {
    for trc in 0..=u8::MAX {
        for b in SPECIAL {
            check_maximum_nits(trc, b);
        }
    }
}

#[test]
fn over_sdr_white_special_pairs() {
    for n in SPECIAL {
        for w in SPECIAL {
            check_over(n, w);
        }
    }
}

#[test]
fn small_inputs_every_value() {
    for v in 0..=u16::MAX {
        // SAFETY: plain values in and out.
        let c = unsafe { oracle::oracle_obs_source_async_upload_hdr_lw(v) };
        assert_eq!(
            c.to_bits(),
            shim::obs_source_async_upload_hdr_lw(v).to_bits(),
            "{v}"
        );
    }
    for flags in 0..=u8::MAX {
        for i in 0..32u8 {
            let bit = |k: u8| i & (1 << k) != 0;
            check_flags(flags, bit(0), bit(1), bit(2), bit(3), bit(4));
        }
    }
    for t in (-100_000..=100_000).chain([i32::MIN, i32::MIN + 1, i32::MAX - 1, i32::MAX]) {
        check_routes(t);
    }
    let ends = [i32::MIN, i32::MIN + 1, i32::MAX - 1, i32::MAX];
    for tex in (-300..=300).chain(ends) {
        for frame in (-300..=300).chain(ends) {
            check_swap(tex, frame);
        }
    }
}

// --- update_async_texrender()'s uniforms in its order -----------------------------

/// The values `update_async_texrender()` sets once `gs_texrender_begin()`
/// succeeds, in its order, as bits: the six size uniforms, the HLG peak (the
/// C computes `hlg_exponent` from it), the maximum nits, the ratio,
/// `hdr_lw`, and whether the range is set.
fn uniforms_c(
    cx: u32,
    cy: u32,
    trc: u8,
    peak: f32,
    white: f32,
    lw: u16,
    full_range: bool,
) -> Vec<u32> {
    // SAFETY: plain values in and out.
    unsafe {
        let s = oracle::oracle_obs_source_async_upload_size_uniforms(cx, cy);
        let p = oracle::oracle_obs_source_async_upload_hlg_peak(peak);
        let n = oracle::oracle_obs_source_async_upload_maximum_nits(trc, p);
        let o = oracle::oracle_obs_source_async_upload_over_sdr_white(n, white);
        let l = oracle::oracle_obs_source_async_upload_hdr_lw(lw);
        let r = oracle::oracle_obs_source_async_upload_sets_range(full_range);
        let mut v = size_bits([
            s.width,
            s.height,
            s.width_d2,
            s.height_d2,
            s.width_x2_i,
            s.height_x2_i,
        ])
        .to_vec();
        v.extend([
            p.to_bits(),
            n.to_bits(),
            o.to_bits(),
            l.to_bits(),
            u32::from(r),
        ]);
        v
    }
}

fn uniforms_rust(
    cx: u32,
    cy: u32,
    trc: u8,
    peak: f32,
    white: f32,
    lw: u16,
    full_range: bool,
) -> Vec<u32> {
    let s = shim::obs_source_async_upload_size_uniforms(cx, cy);
    let p = shim::obs_source_async_upload_hlg_peak(peak);
    let n = shim::obs_source_async_upload_maximum_nits(trc, p);
    let o = shim::obs_source_async_upload_over_sdr_white(n, white);
    let l = shim::obs_source_async_upload_hdr_lw(lw);
    let r = shim::obs_source_async_upload_sets_range(full_range);
    let mut v = size_bits([
        s.width,
        s.height,
        s.width_d2,
        s.height_d2,
        s.width_x2_i,
        s.height_x2_i,
    ])
    .to_vec();
    v.extend([
        p.to_bits(),
        n.to_bits(),
        o.to_bits(),
        l.to_bits(),
        u32::from(r),
    ]);
    v
}

#[test]
fn texrender_uniforms_every_transfer_range_and_level() {
    let sizes = [
        (1u32, 1u32),
        (1920, 1080),
        (1919, 1),
        (0, u32::MAX),
        (16_777_217, 3),
    ];
    let lws = [0u16, 1000, 10000, u16::MAX];
    let mut checked = 0usize;
    for trc in 0..=u8::MAX {
        for full_range in [false, true] {
            for peak in SPECIAL {
                for white in SPECIAL {
                    let i = checked;
                    let (cx, cy) = sizes[i % sizes.len()];
                    let lw = lws[i % lws.len()];
                    let (p, w) = (f32::from_bits(peak), f32::from_bits(white));
                    assert_eq!(
                        uniforms_c(cx, cy, trc, p, w, lw, full_range),
                        uniforms_rust(cx, cy, trc, p, w, lw, full_range),
                        "{cx} x {cy} trc {trc} range {full_range} peak {peak:#010x} white {white:#010x} lw {lw}"
                    );
                    checked += 1;
                }
            }
        }
    }
    assert_eq!(checked, 256 * 2 * SPECIAL.len() * SPECIAL.len());
}

// --- proptests -------------------------------------------------------------------

fn level_bits() -> impl Strategy<Value = u32> {
    prop_oneof![
        any::<u32>(),
        proptest::sample::select(SPECIAL.to_vec()),
        // finite normals of either sign
        (0x0080_0000u32..0x7f80_0000)
            .prop_flat_map(|b| prop_oneof![Just(b), Just(b | 0x8000_0000)]),
        // subnormals
        (1u32..0x0080_0000).prop_flat_map(|b| prop_oneof![Just(b), Just(b | 0x8000_0000)]),
        // NaNs, any payload
        (0x7f80_0001u32..=0x7fff_ffff)
            .prop_flat_map(|b| prop_oneof![Just(b), Just(b | 0x8000_0000)]),
        // levels a user sets
        (1u32..=20_000).prop_map(|n| (n as f32 * 0.5).to_bits()),
    ]
}

fn any_format() -> impl Strategy<Value = ShimFormat> {
    (
        prop_oneof![Just(0u32), Just(1920), Just(u32::MAX), any::<u32>()],
        prop_oneof![Just(0u32), Just(1080), Just(u32::MAX), any::<u32>()],
        prop_oneof![-1..27i32, any::<i32>()],
        any::<bool>(),
        prop_oneof![0..5u8, any::<u8>()],
    )
        .prop_map(|(width, height, format, full_range, trc)| ShimFormat {
            width,
            height,
            format,
            full_range,
            trc,
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20_000))]

    #[test]
    fn prop_texture_changed(cur in any_format(), frame in any_format(), same in any::<bool>()) {
        check_changed(cur, if same { cur } else { frame });
    }

    #[test]
    fn prop_size_uniforms(cx in any::<u32>(), cy in any::<u32>()) {
        check_size(cx, cy);
    }

    #[test]
    fn prop_levels(peak in level_bits(), white in level_bits(), nits in level_bits(), trc in any::<u8>()) {
        check_hlg_peak(peak);
        check_maximum_nits(trc, peak);
        check_over(nits, white);
    }

    #[test]
    fn prop_uniforms(cx in any::<u32>(), cy in any::<u32>(), trc in prop_oneof![0..5u8, any::<u8>()],
                     peak in level_bits(), white in level_bits(), lw in any::<u16>(), full_range in any::<bool>()) {
        let (p, w) = (f32::from_bits(peak), f32::from_bits(white));
        prop_assert_eq!(uniforms_c(cx, cy, trc, p, w, lw, full_range), uniforms_rust(cx, cy, trc, p, w, lw, full_range));
    }

    #[test]
    fn prop_decisions(t in any::<i32>(), tex in prop_oneof![-1..24i32, any::<i32>()],
                      frame in prop_oneof![-1..27i32, any::<i32>()], flags in any::<u8>(),
                      b in any::<[bool; 5]>()) {
        check_routes(t);
        check_swap(tex, frame);
        check_flags(flags, b[0], b[1], b[2], b[3], b[4]);
    }
}
