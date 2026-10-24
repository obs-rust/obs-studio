//! Tier 2 layout and Tier 3 parity: the shims of
//! `obs_core::ffi::video_copy_plan` (and the safe core under them) against
//! the original `libobs/obs-video-copy-plan.c.h`, compiled as
//! `oracle_obs_video_*` (obs-c-oracle, oracle/video_copy_plan.c).
//!
//! Every output is compared exactly: each copy of a plan field by field
//! (the `size_t` sizes included) with the count and the unused slots, the
//! effect as its C value, the close and reuse answers as bools.
//!
//! Runs: the constants and the layout of the four structs; every C format
//! value (the enum, `VIDEO_FORMAT_R10L + 1`, -1, `INT_MIN`, `INT_MAX`)
//! against every pattern of the six line sizes, each equal to its row size,
//! off it, or 0 (729 patterns), over widths and heights at 0, 1, odd, even
//! and the `u32` wrap edges; the RGBX copy over the same sizes with equal
//! and unequal line sizes; the scale effect over every scale type value from
//! -2 to 7 and the `int` ends, all 64 loaded masks of six bits, close or
//! not (+-15, 16, 17 on each axis) and low resolution or not (at the half
//! edges); `resolution_close` around every edge; the reuse match with each
//! field flipped and both rendered flags; and proptests over full-range
//! `u32` sizes and line sizes (biased to the row sizes and to 0),
//! full-range scale inputs and full-range reuse keys.
//!
//! No input is excluded on a 64-bit `long` (Linux, macOS). With a 32-bit
//! `long` (MSVC), `resolution_close()` (and the scale effect through it) is
//! compared with the C only for sizes below 2^31: the C converts a size of
//! 2^31 or more to a negative `long`, and its subtraction can overflow
//! (undefined). The port subtracts as `i64`, which is what the C does with a
//! 64-bit `long` everywhere, and what it does with a 32-bit one for every
//! size below 2^31, so for every size `obs_video_size_valid()` allows and
//! every texture size. That is the one intentional difference, on MSVC only
//! and unreachable; the shim is still compared with the core there.
//! `test_video_copy_plan_steps` runs the C test against the C; the
//! real-libobs video tests run against the hybrid libobs, where these shims
//! replace the C.

use core::ffi::{c_int, c_long, c_void};
use std::mem::{align_of, offset_of, size_of};

use obs_c_oracle::video_copy_plan::{
    CONSTANT_NAMES, LAYOUT_NAMES, OracleCopy, OracleCopyPlan, oracle_obs_video_gpu_copy_plan,
    oracle_obs_video_mix_reuse_match, oracle_obs_video_rgbx_copy_plan,
    oracle_obs_video_scale_effect_choice, oracle_obs_video_scale_resolution_close,
    oracle_video_copy_plan_constant, oracle_video_copy_plan_layout,
};
use obs_core::ffi::video_copy_plan::{
    OBS_VIDEO_SCALE_EFFECT_AREA, OBS_VIDEO_SCALE_EFFECT_BICUBIC,
    OBS_VIDEO_SCALE_EFFECT_BILINEAR_LOWRES, OBS_VIDEO_SCALE_EFFECT_DEFAULT,
    OBS_VIDEO_SCALE_EFFECT_LANCZOS, obs_video_copy, obs_video_copy_plan, obs_video_gpu_copy_plan,
    obs_video_mix_reuse_key, obs_video_mix_reuse_match, obs_video_rgbx_copy_plan,
    obs_video_scale_effect_choice, obs_video_scale_input, obs_video_scale_resolution_close,
    scale_effect_to_c,
};
use obs_core::video_copy_plan::{
    self as plan, COPY_MAX, CopyPlan, FrameCopy, MixReuseKey, ScaleEffect, ScaleInput,
};
use obs_core::video_plan::ObsScaleType;
use obs_media_io::video_io::{VideoFormat, video_format_from_c, video_format_to_c};
use proptest::prelude::*;

fn oracle_copy(c: OracleCopy) -> obs_video_copy {
    obs_video_copy {
        src_after_plane0: c.src_after_plane0,
        whole: c.whole,
        bytes: c.bytes,
        rows: c.rows,
        src_stride: c.src_stride,
        dst_stride: c.dst_stride,
    }
}

fn oracle_plan(p: OracleCopyPlan) -> obs_video_copy_plan {
    obs_video_copy_plan {
        count: p.count,
        copies: p.copies.map(oracle_copy),
    }
}

fn core_copy(c: FrameCopy) -> obs_video_copy {
    obs_video_copy::from(c)
}

fn core_plan(p: CopyPlan) -> obs_video_copy_plan {
    obs_video_copy_plan::from(p)
}

/// The shim, the C and the core on one frame; returns the plan.
fn check_gpu(
    format: c_int,
    width: u32,
    height: u32,
    input: [u32; 8],
    output: [u32; 8],
) -> obs_video_copy_plan {
    // SAFETY: two arrays of MAX_AV_PLANES line sizes, read during the call.
    let rust =
        unsafe { obs_video_gpu_copy_plan(format, width, height, input.as_ptr(), output.as_ptr()) };
    // SAFETY: as above.
    let c = oracle_plan(unsafe {
        oracle_obs_video_gpu_copy_plan(format, width, height, input.as_ptr(), output.as_ptr())
    });
    assert_eq!(
        rust, c,
        "gpu_copy_plan({format}, {width}, {height}, {input:?}, {output:?})"
    );
    let first = |l: [u32; 8]| [l[0], l[1], l[2]];
    let core = plan::gpu_copy_plan(
        video_format_from_c(format),
        width,
        height,
        &first(input),
        &first(output),
    );
    assert_eq!(
        core_plan(core),
        c,
        "core gpu_copy_plan({format}, {width}, {height}, {input:?}, {output:?})"
    );
    assert!(c.count <= COPY_MAX);
    c
}

fn check_rgbx(width: u32, height: u32, l_in: u32, l_out: u32) -> obs_video_copy {
    let rust = obs_video_rgbx_copy_plan(width, height, l_in, l_out);
    // SAFETY: a plain value function.
    let c = oracle_copy(unsafe { oracle_obs_video_rgbx_copy_plan(width, height, l_in, l_out) });
    assert_eq!(
        rust, c,
        "rgbx_copy_plan({width}, {height}, {l_in}, {l_out})"
    );
    assert_eq!(
        core_copy(plan::rgbx_copy_plan(width, height, l_in, l_out)),
        c,
        "core rgbx_copy_plan({width}, {height}, {l_in}, {l_out})"
    );
    c
}

/// Whether the C's `resolution_close()` is defined, and so comparable, for
/// these sizes: always with a 64-bit `long` (LP64: Linux, macOS); with a
/// 32-bit one (LLP64: MSVC) only below 2^31, since `(long)size` makes a size
/// of 2^31 or more negative and the subtraction can then overflow. Every
/// size `obs_video_size_valid()` allows (32768 at most) and every texture
/// size is far below. Above it the shim is still compared with the core.
fn c_long_holds(sizes: [u32; 4]) -> bool {
    size_of::<c_long>() >= 8 || sizes.iter().all(|&s| s < 0x8000_0000)
}

fn check_close(bw: u32, bh: u32, w: u32, h: u32) -> bool {
    let rust = obs_video_scale_resolution_close(bw, bh, w, h);
    assert_eq!(plan::resolution_close(bw, bh, w, h), rust, "core");
    if !c_long_holds([bw, bh, w, h]) {
        return rust;
    }
    // SAFETY: a plain value function.
    let c = unsafe { oracle_obs_video_scale_resolution_close(bw, bh, w, h) };
    assert_eq!(rust, c, "resolution_close({bw}, {bh}, {w}, {h})");
    c
}

fn check_effect(input: obs_video_scale_input) -> u32 {
    // SAFETY: a live input, read during the call.
    let rust = unsafe { obs_video_scale_effect_choice(&raw const input) };
    let c = if c_long_holds([
        input.base_width,
        input.base_height,
        input.target_width,
        input.target_height,
    ]) {
        // SAFETY: as above; the oracle reads it as struct
        // obs_video_scale_input, whose layout layout_matches_c checks.
        unsafe { oracle_obs_video_scale_effect_choice((&raw const input).cast::<c_void>()) }
    } else {
        rust
    };
    assert_eq!(rust, c, "scale_effect_choice({input:?})");
    let core = plan::scale_effect(&ScaleInput {
        base_width: input.base_width,
        base_height: input.base_height,
        target_width: input.target_width,
        target_height: input.target_height,
        output_width: input.output_width,
        output_height: input.output_height,
        scale_type: ObsScaleType::from_c(input.scale_type),
        loaded: input.loaded,
    });
    assert_eq!(scale_effect_to_c(core), c, "core {input:?}");
    c
}

fn check_reuse(other: obs_video_mix_reuse_key, mix: obs_video_mix_reuse_key) -> bool {
    // SAFETY: two live keys, read during the call.
    let rust = unsafe { obs_video_mix_reuse_match(&raw const other, &raw const mix) };
    // SAFETY: as above; the layout is checked against the C.
    let c = unsafe {
        oracle_obs_video_mix_reuse_match(
            (&raw const other).cast::<c_void>(),
            (&raw const mix).cast::<c_void>(),
        )
    };
    assert_eq!(rust, c, "mix_reuse_match({other:?}, {mix:?})");
    assert_eq!(
        plan::mix_reuse_match(&MixReuseKey::from(other), &MixReuseKey::from(mix)),
        c,
        "core"
    );
    c
}

// ---------------------------------------------------------------------------
// layout and constants

#[test]
fn constants_match_c() {
    let rust: [i64; 16] = [
        COPY_MAX as i64,
        i64::from(ObsScaleType::Disable.to_c()),
        i64::from(ObsScaleType::Point.to_c()),
        i64::from(ObsScaleType::Bicubic.to_c()),
        i64::from(ObsScaleType::Bilinear.to_c()),
        i64::from(ObsScaleType::Lanczos.to_c()),
        i64::from(ObsScaleType::Area.to_c()),
        i64::from(OBS_VIDEO_SCALE_EFFECT_DEFAULT),
        i64::from(OBS_VIDEO_SCALE_EFFECT_BILINEAR_LOWRES),
        i64::from(OBS_VIDEO_SCALE_EFFECT_BICUBIC),
        i64::from(OBS_VIDEO_SCALE_EFFECT_LANCZOS),
        i64::from(OBS_VIDEO_SCALE_EFFECT_AREA),
        i64::from(ScaleEffect::Default.loaded_bit()),
        i64::from(ScaleEffect::Area.loaded_bit()),
        8,
        i64::from(video_format_to_c(VideoFormat::R10l)),
    ];
    for (i, (name, want)) in CONSTANT_NAMES.iter().zip(rust).enumerate() {
        // SAFETY: a plain value function.
        let c = unsafe { oracle_video_copy_plan_constant(i) };
        assert_eq!(want, c, "{name}");
    }
    // SAFETY: as above.
    assert_eq!(
        unsafe { oracle_video_copy_plan_constant(CONSTANT_NAMES.len()) },
        -1
    );
    for effect in ScaleEffect::ALL {
        let value = scale_effect_to_c(effect);
        assert_eq!(effect.loaded_bit(), 1 << value, "{effect:?}");
    }
}

#[test]
fn layout_matches_c() {
    let rust: [usize; 31] = [
        size_of::<obs_video_copy>(),
        align_of::<obs_video_copy>(),
        offset_of!(obs_video_copy, src_after_plane0),
        offset_of!(obs_video_copy, whole),
        offset_of!(obs_video_copy, bytes),
        offset_of!(obs_video_copy, rows),
        offset_of!(obs_video_copy, src_stride),
        offset_of!(obs_video_copy, dst_stride),
        size_of::<obs_video_copy_plan>(),
        align_of::<obs_video_copy_plan>(),
        offset_of!(obs_video_copy_plan, count),
        offset_of!(obs_video_copy_plan, copies),
        size_of::<obs_video_scale_input>(),
        align_of::<obs_video_scale_input>(),
        offset_of!(obs_video_scale_input, base_width),
        offset_of!(obs_video_scale_input, base_height),
        offset_of!(obs_video_scale_input, target_width),
        offset_of!(obs_video_scale_input, target_height),
        offset_of!(obs_video_scale_input, output_width),
        offset_of!(obs_video_scale_input, output_height),
        offset_of!(obs_video_scale_input, scale_type),
        offset_of!(obs_video_scale_input, loaded),
        size_of::<obs_video_mix_reuse_key>(),
        align_of::<obs_video_mix_reuse_key>(),
        offset_of!(obs_video_mix_reuse_key, view),
        offset_of!(obs_video_mix_reuse_key, render_space),
        offset_of!(obs_video_mix_reuse_key, base_width),
        offset_of!(obs_video_mix_reuse_key, base_height),
        offset_of!(obs_video_mix_reuse_key, rendered),
        size_of::<u32>(),
        size_of::<c_int>(),
    ];
    for (i, (name, want)) in LAYOUT_NAMES.iter().zip(rust).enumerate() {
        // SAFETY: a plain value function.
        let c = unsafe { oracle_video_copy_plan_layout(i) };
        assert_eq!(want, c, "{name}");
    }
    // the oracle's own structs too
    assert_eq!(size_of::<OracleCopy>(), size_of::<obs_video_copy>());
    assert_eq!(
        size_of::<OracleCopyPlan>(),
        size_of::<obs_video_copy_plan>()
    );
}

// ---------------------------------------------------------------------------
// the copy plan

/// Every C value of `enum video_format`, and values outside it.
fn every_format() -> Vec<c_int> {
    VideoFormat::ALL
        .into_iter()
        .map(video_format_to_c)
        .chain([
            video_format_to_c(VideoFormat::R10l) + 1,
            -1,
            c_int::MIN,
            c_int::MAX,
        ])
        .collect()
}

const EDGE_SIZES: [u32; 14] = [
    0,
    1,
    2,
    3,
    359,
    640,
    1081,
    1920,
    0x4000_0000,
    0x7fff_ffff,
    0x8000_0000,
    0x8000_0001,
    0xffff_fffe,
    0xffff_ffff,
];

/// The row sizes a plane can have at `width`, as the C computes them.
fn row_sizes(width: u32) -> [u32; 4] {
    [
        width,
        width / 2,
        width.wrapping_mul(2),
        width.wrapping_mul(4),
    ]
}

#[test]
fn gpu_plan_every_format_and_line_size_pattern() {
    let mut calls = 0u64;
    let mut whole = 0u64;
    let mut packed = 0u64;
    for format in every_format() {
        for &w in &EDGE_SIZES {
            for &h in &[0u32, 1, 3, 1080, 0xffff_ffff] {
                // each of the six line sizes: the plane's row size (one of
                // the four the C makes, chosen per plane), off it, or 0
                let rows = row_sizes(w);
                for pattern in 0..729u32 {
                    let mut input = [0u32; 8];
                    let mut output = [0u32; 8];
                    let mut p = pattern;
                    for l in 0..6 {
                        let choice = p % 3;
                        p /= 3;
                        let tight = rows[(l + pattern as usize) % 4];
                        let value = match choice {
                            0 => tight,
                            1 => tight.wrapping_add(64),
                            _ => 0,
                        };
                        if l < 3 {
                            input[l] = value;
                        } else {
                            output[l - 3] = value;
                        }
                    }
                    // also the frames' other line sizes, which are not read
                    input[3..].fill(0xdead_beef);
                    output[3..].fill(0xfeed_f00d);
                    let c = check_gpu(format, w, h, input, output);
                    calls += 1;
                    whole += c.copies.iter().filter(|c| c.whole).count() as u64;
                    packed += u64::from(c.copies[1].src_after_plane0);
                }
            }
        }
    }
    // every kind of copy was reached
    assert!(whole > 0 && packed > 0, "{whole} {packed}");
    assert_eq!(calls, 30 * 14 * 5 * 729);
}

#[test]
fn gpu_plan_tight_planes_every_format() {
    // the line sizes a real frame has: each plane's own row size
    for format in every_format() {
        for &w in &EDGE_SIZES {
            for &h in &EDGE_SIZES {
                let r = row_sizes(w);
                for (input, output) in [
                    ([r[0], r[1], r[1]], [r[0], r[1], r[1]]),
                    ([r[0], r[0], 0], [r[0], r[0], 0]),
                    ([r[0], 0, 0], [r[0], r[0], 0]),
                    ([r[2], r[0], r[0]], [r[2], r[0], r[0]]),
                    ([r[2], r[2], 0], [r[2], r[2], 0]),
                    ([r[2], 0, 0], [r[2], r[2], 0]),
                    ([r[2], r[3], 0], [r[2], r[3], 0]),
                    ([r[0], r[0], r[0]], [r[0], r[0], r[0]]),
                ] {
                    let mut i8 = [0u32; 8];
                    let mut o8 = [0u32; 8];
                    i8[..3].copy_from_slice(&input);
                    o8[..3].copy_from_slice(&output);
                    check_gpu(format, w, h, i8, o8);
                }
            }
        }
    }
}

#[test]
fn rgbx_plan_edges() {
    for &w in &EDGE_SIZES {
        for &h in &EDGE_SIZES {
            for &l in &EDGE_SIZES {
                check_rgbx(w, h, l, l);
                check_rgbx(w, h, l, l.wrapping_add(1));
                check_rgbx(w, h, l, w.wrapping_mul(4));
                check_rgbx(w, h, w.wrapping_mul(4), w.wrapping_mul(4));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// the scale effect

const EVERY_SCALE_TYPE: [c_int; 12] = [-2, -1, 0, 1, 2, 3, 4, 5, 6, 7, c_int::MIN, c_int::MAX];

#[test]
fn scale_effect_every_type_mask_close_and_lowres() {
    let mut seen = [0u32; 5];
    for &(bw, bh) in &[
        (1920u32, 1080u32),
        (1921, 1081),
        (2, 2),
        (32768, 32768),
        (0, 0),
        (1, 1),
    ] {
        // targets: close (+-15, 16) or not (+-17, far)
        for &(dw, dh) in &[
            (0i64, 0i64),
            (15, -15),
            (16, 16),
            (-16, 0),
            (17, 0),
            (0, -17),
            (17, 17),
            (-1000, -500),
        ] {
            let tw = u32::try_from(i64::from(bw) + dw).unwrap_or(0);
            let th = u32::try_from(i64::from(bh) + dh).unwrap_or(0);
            // outputs at the half edges: under half on both axes, on one
            // axis only, at half, and 0
            for &(ow, oh) in &[
                ((bw / 2).saturating_sub(1), (bh / 2).saturating_sub(1)),
                (bw / 2, (bh / 2).saturating_sub(1)),
                ((bw / 2).saturating_sub(1), bh / 2),
                (bw / 2, bh / 2),
                (0, 0),
                (bw, bh),
                (u32::MAX, u32::MAX),
            ] {
                for &scale_type in &EVERY_SCALE_TYPE {
                    for loaded in (0..64u32).chain([0xffff_ffe0, u32::MAX]) {
                        let e = check_effect(obs_video_scale_input {
                            base_width: bw,
                            base_height: bh,
                            target_width: tw,
                            target_height: th,
                            output_width: ow,
                            output_height: oh,
                            scale_type,
                            loaded,
                        });
                        seen[e as usize] += 1;
                    }
                }
            }
        }
    }
    // every effect was chosen
    assert!(seen.iter().all(|&n| n > 0), "{seen:?}");
}

#[test]
fn resolution_close_edges() {
    let mut pairs = 0u32;
    for &base in &EDGE_SIZES {
        for d in -40i64..=40 {
            let s = u32::try_from(i64::from(base) + d).unwrap_or(base);
            for &other in &[0u32, base, s, u32::MAX] {
                check_close(base, other, s, other);
                check_close(other, base, other, s);
                check_close(base, base, s, s);
                pairs += 3;
            }
        }
    }
    assert!(pairs > 10_000);
}

// ---------------------------------------------------------------------------
// the reuse match

#[test]
fn reuse_every_flip() {
    for &(view, space, w, h) in &[
        (0x1000usize, 0 as c_int, 1920u32, 1080u32),
        (0, 0, 0, 0),
        (usize::MAX, -1, u32::MAX, u32::MAX),
        (1, c_int::MIN, 1, 0x8000_0000),
    ] {
        for flips in 0..16u32 {
            for rendered in [false, true] {
                for mix_rendered in [false, true] {
                    let other = obs_video_mix_reuse_key {
                        view,
                        render_space: space,
                        base_width: w,
                        base_height: h,
                        rendered,
                    };
                    let mut mix = obs_video_mix_reuse_key {
                        rendered: mix_rendered,
                        ..other
                    };
                    if flips & 1 != 0 {
                        mix.view ^= 1 << 12;
                    }
                    if flips & 2 != 0 {
                        mix.render_space = mix.render_space.wrapping_add(1);
                    }
                    if flips & 4 != 0 {
                        mix.base_width ^= 1;
                    }
                    if flips & 8 != 0 {
                        mix.base_height ^= 0x8000_0000;
                    }
                    let got = check_reuse(other, mix);
                    assert_eq!(got, flips == 0 && rendered);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// proptests

fn size() -> impl Strategy<Value = u32> {
    prop_oneof![
        3 => any::<u32>(),
        1 => prop::sample::select(EDGE_SIZES.to_vec()),
        1 => 0u32..4096,
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(8192))]

    #[test]
    fn gpu_plan_full_range(
        f in prop::sample::select(every_format()),
        w in size(),
        h in size(),
        raw_in: [u32; 8],
        raw_out: [u32; 8],
        bias in any::<u16>(),
    ) {
        // bias line sizes to a row size (so the single copy is taken) or 0
        let rows = row_sizes(w);
        let mut input = raw_in;
        let mut output = raw_out;
        for l in 0..3 {
            let pick = |k: u16| rows[usize::from(k) % 4];
            match (bias >> (l * 4)) & 3 {
                0 => input[l] = pick(bias >> (l * 4 + 2)),
                1 => input[l] = 0,
                _ => {}
            }
            match (bias >> (l * 4 + 1)) & 3 {
                0 => output[l] = pick(bias >> (l * 4 + 3)),
                1 => output[l] = 0,
                _ => {}
            }
        }
        check_gpu(f, w, h, input, output);
    }

    #[test]
    fn rgbx_plan_full_range(w in size(), h in size(), l_in in size(), l_out in size(), same: bool) {
        check_rgbx(w, h, l_in, if same { l_in } else { l_out });
    }

    #[test]
    fn scale_effect_full_range(
        bw in size(), bh in size(), tw in size(), th in size(), ow in size(), oh in size(),
        near: bool,
        dw in -20i64..=20, dh in -20i64..=20,
        scale_type: c_int,
        small_type in -2 as c_int..8,
        use_small: bool,
        loaded: u32,
    ) {
        let (tw, th) = if near {
            (u32::try_from(i64::from(bw) + dw).unwrap_or(tw), u32::try_from(i64::from(bh) + dh).unwrap_or(th))
        } else {
            (tw, th)
        };
        check_effect(obs_video_scale_input {
            base_width: bw,
            base_height: bh,
            target_width: tw,
            target_height: th,
            output_width: ow,
            output_height: oh,
            scale_type: if use_small { small_type } else { scale_type },
            loaded,
        });
    }

    #[test]
    fn resolution_close_full_range(bw in size(), bh in size(), w in size(), h in size(), d in -20i64..=20) {
        check_close(bw, bh, w, h);
        let near = u32::try_from(i64::from(bw) + d).unwrap_or(bw);
        check_close(bw, bh, near, bh);
    }

    #[test]
    fn reuse_full_range(
        view: usize, space: c_int, w: u32, h: u32, rendered: bool,
        mix_view: usize, mix_space: c_int, mix_w: u32, mix_h: u32, mix_rendered: bool,
        same in 0u32..32,
    ) {
        let other = obs_video_mix_reuse_key { view, render_space: space, base_width: w, base_height: h, rendered };
        let mix = obs_video_mix_reuse_key {
            view: if same & 1 != 0 { view } else { mix_view },
            render_space: if same & 2 != 0 { space } else { mix_space },
            base_width: if same & 4 != 0 { w } else { mix_w },
            base_height: if same & 8 != 0 { h } else { mix_h },
            rendered: mix_rendered,
        };
        check_reuse(other, mix);
    }
}
