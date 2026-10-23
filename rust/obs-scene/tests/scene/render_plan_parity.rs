//! Tier 3: the shims of `obs_scene::ffi::scene_render_plan` against the
//! original `obs-scene-render-plan.c.h` (the oracle).
//!
//! Every output is compared field by field, bit for bit: the bools, the
//! enum values as integers, and the multiplier as `f32` bits, so -0 against
//! +0, a different NaN or a different rounding would fail. The matrices
//! and floats are drawn from bit patterns, so -0.0 and NaN payloads (quiet
//! and signalling, of either sign) are covered, never from a wide `f32`
//! range (#1951).
//!
//! Runs:
//! - the layouts of both structs, `struct matrix4` and the enum sizes, and
//!   every enum value and mirrored constant, against the C header;
//! - exhaustive: the texrender action and the child over all 8 rows each;
//!   the triggers over every zero/non-zero pattern of the eight crops, with
//!   11 scale filters, 6 blend methods, 11 blend types and the two flags
//!   (743,424 calls); the size check over the `u32` edges; the clear colour
//!   and the linear sRGB state over every blend value from -2 to 9 and the
//!   ends of `i32`; the technique and the multiplier over every pair of
//!   spaces from -2 to 6 and the ends of `i32` (with 26 white levels); the
//!   effect over 15 filters and every pair of 86 scales taken from around
//!   0.5, 1 - EPSILON, 1, 1 + EPSILON, 0, the infinities and the NaNs
//!   (110,940 calls); and the centred check with each of the 16 entries
//!   set to 30 special values, and every pair of them as the translation;
//! - proptests (10,000 cases each): the triggers over full `i32` fields,
//!   the effect over any `i32` filter and any-bit scales biased to those
//!   edges, the multiplier over any spaces and any-bit white level, and the
//!   centred check over matrices whose entries are the identity's, a whole
//!   or fractional translation, or any bits.
//!
//! No intentional differences. `test_scene_render_plan_steps` compiles the
//! C into itself, so this is what compares the swap;
//! `test_scene_render_plan_steps_rust` runs the same C test against the
//! shims, and the real-libobs scene tests see it.

use std::mem::{align_of, offset_of, size_of};

use obs_c_oracle::graphics_math::{OracleMatrix4, OracleVec4};
use obs_c_oracle::scene_render_plan::{
    LAYOUT_COUNT, oracle_obs_scene_render_child, oracle_obs_scene_render_clear_white,
    oracle_obs_scene_render_effect, oracle_obs_scene_render_linear_srgb,
    oracle_obs_scene_render_multiplier, oracle_obs_scene_render_size_empty,
    oracle_obs_scene_render_technique, oracle_obs_scene_render_texrender_action,
    oracle_obs_scene_render_texture_enabled, oracle_obs_scene_texcoords_centered,
    oracle_scene_render_effect_plan, oracle_scene_render_plan_layout, oracle_scene_render_triggers,
};
use obs_graphics::ffi::matrix4::matrix4;
use obs_graphics::ffi::vec4::vec4;
use obs_scene::ffi::scene_render_plan as shim;
use obs_scene::ffi::scene_render_plan::{obs_scene_render_effect_plan, obs_scene_render_triggers};
use obs_scene::render_plan as core;
use proptest::prelude::*;

const INT_MAX: i32 = i32::MAX;
const INT_MIN: i32 = i32::MIN;

// ---------------------------------------------------------------------------
// Callers: one shim call and one oracle call, compared

fn oracle_triggers(t: &obs_scene_render_triggers) -> oracle_scene_render_triggers {
    oracle_scene_render_triggers {
        crop_left: t.crop_left,
        crop_top: t.crop_top,
        crop_right: t.crop_right,
        crop_bottom: t.crop_bottom,
        bounds_crop_left: t.bounds_crop_left,
        bounds_crop_top: t.bounds_crop_top,
        bounds_crop_right: t.bounds_crop_right,
        bounds_crop_bottom: t.bounds_crop_bottom,
        scale_filter: t.scale_filter,
        blend_method: t.blend_method,
        blend_type: t.blend_type,
        is_scene: t.is_scene,
        is_group: t.is_group,
    }
}

fn check_triggers(t: &obs_scene_render_triggers) {
    // SAFETY: both read a local struct.
    let rust = unsafe { shim::obs_scene_render_texture_enabled(t) };
    let o = oracle_triggers(t);
    // SAFETY: as above.
    let c = unsafe { oracle_obs_scene_render_texture_enabled(&o) };
    assert_eq!(rust, c, "texture_enabled({t:?})");
}

fn check_effect(filter: i32, sx: f32, sy: f32) {
    let mut rust = obs_scene_render_effect_plan {
        effect: -7,
        point_sampler: true,
        base_dimension: true,
        upscale: true,
    };
    let mut c = oracle_scene_render_effect_plan {
        effect: -9,
        point_sampler: false,
        base_dimension: false,
        upscale: false,
    };
    // SAFETY: both write a local struct.
    unsafe {
        shim::obs_scene_render_effect(filter, sx, sy, &raw mut rust);
        oracle_obs_scene_render_effect(filter, sx, sy, &raw mut c);
    }
    let what = || {
        format!(
            "effect({filter}, {sx} [{:#010x}], {sy} [{:#010x}])",
            sx.to_bits(),
            sy.to_bits()
        )
    };
    assert_eq!(rust.effect, c.effect, "{}: effect", what());
    assert_eq!(rust.point_sampler, c.point_sampler, "{}: point", what());
    assert_eq!(rust.base_dimension, c.base_dimension, "{}: base", what());
    assert_eq!(rust.upscale, c.upscale, "{}: upscale", what());
}

fn check_multiplier(current: i32, source: i32, white: f32) {
    let rust = shim::obs_scene_render_multiplier(current, source, white);
    // SAFETY: plain values.
    let c = unsafe { oracle_obs_scene_render_multiplier(current, source, white) };
    assert!(
        rust.to_bits() == c.to_bits(),
        "multiplier({current}, {source}, {white} [{:#010x}]): rust {rust} [{:#010x}] oracle {c} [{:#010x}]",
        white.to_bits(),
        rust.to_bits(),
        c.to_bits()
    );
}

fn check_technique(upscale: bool, current: i32, source: i32) {
    let rust = shim::obs_scene_render_technique(upscale, current, source);
    // SAFETY: plain values.
    let c = unsafe { oracle_obs_scene_render_technique(upscale, current, source) };
    assert_eq!(rust, c, "technique({upscale}, {current}, {source})");
}

fn check_centered(bits: &[u32; 16]) {
    let f = |i: usize| f32::from_bits(bits[i]);
    let row = |i: usize| vec4 {
        x: f(i),
        y: f(i + 1),
        z: f(i + 2),
        w: f(i + 3),
    };
    let orow = |i: usize| OracleVec4 {
        x: f(i),
        y: f(i + 1),
        z: f(i + 2),
        w: f(i + 3),
    };
    let m = matrix4 {
        x: row(0),
        y: row(4),
        z: row(8),
        t: row(12),
    };
    let om = OracleMatrix4 {
        x: orow(0),
        y: orow(4),
        z: orow(8),
        t: orow(12),
    };
    // SAFETY: both read a local matrix.
    let rust = unsafe { shim::obs_scene_texcoords_centered(&m) };
    // SAFETY: as above.
    let c = unsafe { oracle_obs_scene_texcoords_centered(&om) };
    assert_eq!(rust, c, "texcoords_centered({bits:08x?})");
}

const IDENTITY: [u32; 16] = [
    0x3f80_0000,
    0,
    0,
    0,
    0,
    0x3f80_0000,
    0,
    0,
    0,
    0,
    0x3f80_0000,
    0,
    0,
    0,
    0,
    0x3f80_0000,
];

// ---------------------------------------------------------------------------
// Layouts and constants

fn layout(which: usize) -> usize {
    // SAFETY: plain values.
    unsafe { oracle_scene_render_plan_layout(which) }
}

#[test]
fn layouts_and_constants_match_the_header() {
    type T = obs_scene_render_triggers;
    type P = obs_scene_render_effect_plan;
    let ours: [usize; LAYOUT_COUNT] = [
        size_of::<T>(),
        align_of::<T>(),
        offset_of!(T, crop_left),
        offset_of!(T, crop_top),
        offset_of!(T, crop_right),
        offset_of!(T, crop_bottom),
        offset_of!(T, bounds_crop_left),
        offset_of!(T, bounds_crop_top),
        offset_of!(T, bounds_crop_right),
        offset_of!(T, bounds_crop_bottom),
        offset_of!(T, scale_filter),
        offset_of!(T, blend_method),
        offset_of!(T, blend_type),
        offset_of!(T, is_scene),
        offset_of!(T, is_group),
        size_of::<P>(),
        align_of::<P>(),
        offset_of!(P, effect),
        offset_of!(P, point_sampler),
        offset_of!(P, base_dimension),
        offset_of!(P, upscale),
        size_of::<matrix4>(),
        align_of::<matrix4>(),
        offset_of!(matrix4, t),
        size_of::<shim::obs_scene_render_enum>(),
        size_of::<shim::obs_scene_render_enum>(),
        size_of::<std::ffi::c_int>(),
        size_of::<shim::obs_scene_render_enum>(),
        shim::OBS_SCENE_RENDER_TEXRENDER_NONE as usize,
        shim::OBS_SCENE_RENDER_TEXRENDER_KEEP as usize,
        shim::OBS_SCENE_RENDER_TEXRENDER_DESTROY as usize,
        shim::OBS_SCENE_RENDER_TEXRENDER_CREATE as usize,
        shim::OBS_SCENE_RENDER_TEXRENDER_RECREATE as usize,
        shim::OBS_SCENE_RENDER_CHILD_SOURCE as usize,
        shim::OBS_SCENE_RENDER_CHILD_SHOW as usize,
        shim::OBS_SCENE_RENDER_CHILD_HIDE as usize,
        shim::OBS_SCENE_RENDER_EFFECT_DEFAULT as usize,
        shim::OBS_SCENE_RENDER_EFFECT_BILINEAR_LOWRES as usize,
        shim::OBS_SCENE_RENDER_EFFECT_BICUBIC as usize,
        shim::OBS_SCENE_RENDER_EFFECT_LANCZOS as usize,
        shim::OBS_SCENE_RENDER_EFFECT_AREA as usize,
        shim::OBS_SCENE_RENDER_TECH_DRAW as usize,
        shim::OBS_SCENE_RENDER_TECH_DRAW_MULTIPLY as usize,
        shim::OBS_SCENE_RENDER_TECH_DRAW_TONEMAP as usize,
        shim::OBS_SCENE_RENDER_TECH_DRAW_MULTIPLY_TONEMAP as usize,
        shim::OBS_SCENE_RENDER_TECH_DRAW_UPSCALE as usize,
        shim::OBS_SCENE_RENDER_TECH_DRAW_UPSCALE_MULTIPLY as usize,
        shim::OBS_SCENE_RENDER_TECH_DRAW_UPSCALE_TONEMAP as usize,
        shim::OBS_SCENE_RENDER_TECH_DRAW_UPSCALE_MULTIPLY_TONEMAP as usize,
        core::SCALE_DISABLE as usize,
        core::SCALE_POINT as usize,
        core::SCALE_BICUBIC as usize,
        core::SCALE_LANCZOS as usize,
        core::SCALE_AREA as usize,
        core::BLEND_METHOD_SRGB_OFF as usize,
        core::BLEND_NORMAL as usize,
        core::BLEND_DARKEN as usize,
        core::CS_709_EXTENDED as usize,
        core::CS_709_SCRGB as usize,
    ];
    for (which, want) in ours.iter().enumerate() {
        assert_eq!(layout(which), *want, "layout value {which}");
    }
    assert_eq!(layout(LAYOUT_COUNT), 0, "one past the table");
    // The oracle's own matrix4 declaration agrees with the shim's.
    assert_eq!(size_of::<OracleMatrix4>(), size_of::<matrix4>());
    assert_eq!(align_of::<OracleMatrix4>(), align_of::<matrix4>());
    assert_eq!(size_of::<oracle_scene_render_triggers>(), size_of::<T>());
    assert_eq!(size_of::<oracle_scene_render_effect_plan>(), size_of::<P>());
    // The constants the safe core writes without the C header.
    assert_eq!(core::SCALE_BILINEAR, 3);
    assert_eq!(core::CS_SRGB, 0);
    assert_eq!(core::CS_SRGB_16F, 1);
    assert_eq!(core::EPSILON.to_bits(), 1e-4f32.to_bits());
}

// ---------------------------------------------------------------------------
// Exhaustive

#[test]
fn texrender_and_child_rows_match_oracle() {
    for a in [false, true] {
        for b in [false, true] {
            for c in [false, true] {
                let rust = shim::obs_scene_render_texrender_action(a, b, c);
                // SAFETY: plain values.
                let o = unsafe { oracle_obs_scene_render_texrender_action(a, b, c) };
                assert_eq!(rust, o, "texrender_action({a}, {b}, {c})");
                let rust = shim::obs_scene_render_child(a, b, c);
                // SAFETY: plain values.
                let o = unsafe { oracle_obs_scene_render_child(a, b, c) };
                assert_eq!(rust, o, "child({a}, {b}, {c})");
            }
        }
    }
}

/// The non-zero crop each field gets when it is set.
const CROP_VALUES: [i32; 8] = [1, -1, INT_MAX, INT_MIN, 7, -300, 65536, 2];

#[test]
fn triggers_every_combination_match_oracle() {
    let filters = [-1, 0, 1, 2, 3, 4, 5, 6, 7, INT_MAX, INT_MIN];
    let methods = [-1, 0, 1, 2, INT_MAX, INT_MIN];
    let blends = [-1, 0, 1, 2, 3, 4, 5, 6, 7, INT_MAX, INT_MIN];
    let mut calls = 0usize;
    for pattern in 0u32..256 {
        let crop = |side: u32| {
            if pattern & (1 << side) != 0 {
                CROP_VALUES[side as usize]
            } else {
                0
            }
        };
        for &scale_filter in &filters {
            for &blend_method in &methods {
                for &blend_type in &blends {
                    for is_scene in [false, true] {
                        for is_group in [false, true] {
                            check_triggers(&obs_scene_render_triggers {
                                crop_left: crop(0),
                                crop_top: crop(1),
                                crop_right: crop(2),
                                crop_bottom: crop(3),
                                bounds_crop_left: crop(4),
                                bounds_crop_top: crop(5),
                                bounds_crop_right: crop(6),
                                bounds_crop_bottom: crop(7),
                                scale_filter,
                                blend_method,
                                blend_type,
                                is_scene,
                                is_group,
                            });
                            calls += 1;
                        }
                    }
                }
            }
        }
    }
    assert_eq!(calls, 256 * 11 * 6 * 11 * 4);
}

#[test]
fn scalars_match_oracle() {
    let sizes = [0u32, 1, 2, 1080, u32::MAX - 1, u32::MAX];
    for &w in &sizes {
        for &h in &sizes {
            let rust = shim::obs_scene_render_size_empty(w, h);
            // SAFETY: plain values.
            let c = unsafe { oracle_obs_scene_render_size_empty(w, h) };
            assert_eq!(rust, c, "size_empty({w}, {h})");
        }
    }
    let values = (-2..10).chain([INT_MAX, INT_MIN, INT_MAX - 1, INT_MIN + 1]);
    for v in values {
        let rust = shim::obs_scene_render_clear_white(v);
        // SAFETY: plain values.
        let c = unsafe { oracle_obs_scene_render_clear_white(v) };
        assert_eq!(rust, c, "clear_white({v})");
        for has in [false, true] {
            let rust = shim::obs_scene_render_linear_srgb(has, v);
            // SAFETY: plain values.
            let c = unsafe { oracle_obs_scene_render_linear_srgb(has, v) };
            assert_eq!(rust, c, "linear_srgb({has}, {v})");
        }
    }
}

const SPACES: [i32; 11] = [-2, -1, 0, 1, 2, 3, 4, 5, 6, INT_MAX, INT_MIN];

/// White levels: the usual ones, 0 of either sign, the edges and NaNs.
const WHITES: [u32; 26] = [
    0x42a0_0000, // 80
    0x4396_0000, // 300
    0x4348_0000, // 200
    0x447a_0000, // 1000
    0x3f80_0000, // 1
    0x0000_0000,
    0x8000_0000,
    0x0000_0001,
    0x8000_0001,
    0x007f_ffff,
    0x0080_0000,
    0x7f7f_ffff,
    0xff7f_ffff,
    0x7f80_0000,
    0xff80_0000,
    0x7fc0_0000,
    0xffc0_0000,
    0x7fc0_0001,
    0x7fa0_0000,
    0xffa0_0001,
    0x7f80_0001,
    0x42a0_0001,
    0x429f_ffff,
    0xc2a0_0000,
    0x3e88_8889,
    0x3400_0000,
];

#[test]
fn technique_and_multiplier_match_oracle() {
    for &current in &SPACES {
        for &source in &SPACES {
            for upscale in [false, true] {
                check_technique(upscale, current, source);
            }
            for &white in &WHITES {
                check_multiplier(current, source, f32::from_bits(white));
            }
        }
    }
}

/// Bit patterns within `radius` of `center`, as far as they stay in the
/// same sign.
fn around(center: u32, radius: u32) -> impl Iterator<Item = u32> {
    let sign = center & 0x8000_0000;
    let lo = center.saturating_sub(radius).max(sign);
    let hi = center.saturating_add(radius).min(sign | 0x7fff_ffff);
    lo..=hi
}

/// 86 scales: around 0.5, 1 - EPSILON, 1, 1 + EPSILON, 0 of either sign,
/// the infinities, the largest float and the NaNs.
fn edge_scales() -> Vec<f32> {
    let mut bits: Vec<u32> = Vec::new();
    for center in [
        0x3f00_0000u32, // 0.5
        0x3f7f_f972,    // 1 - EPSILON
        0x3f7f_f973,    // the first close below 1
        0x3f80_0000,    // 1
        0x3f80_0346,    // the last close above 1
        0x3f80_0347,    // 1 + EPSILON
    ] {
        bits.extend(around(center, 6));
    }
    bits.extend(around(0, 4));
    bits.extend(around(0x8000_0000, 4));
    bits.extend(around(0x7f80_0000, 3)); // MAX, inf, sNaN
    bits.extend(around(0xff80_0000, 3));
    bits.extend([
        0x7fc0_0000,
        0xffc0_0000,
        0x7fc1_2345,
        0xbf80_0000, // -1
        0x4000_0000, // 2
        0x3e80_0000, // 0.25
        0x3f40_0000, // 0.75
        0x3c23_d70a, // 0.01
    ]);
    bits.sort_unstable();
    bits.dedup();
    bits.into_iter().map(f32::from_bits).collect()
}

const FILTERS: [i32; 15] = [-2, -1, 0, 1, 2, 3, 4, 5, 6, 7, 8, INT_MAX, INT_MIN, 100, 9];

#[test]
fn effect_edges_match_oracle() {
    let scales = edge_scales();
    assert_eq!(scales.len(), 86);
    for &filter in &FILTERS {
        for &sx in &scales {
            for &sy in &scales {
                check_effect(filter, sx, sy);
            }
        }
    }
}

/// 30 entry values: the identity's, whole and fractional numbers, -0.0,
/// subnormals, the infinities and NaNs (quiet and signalling, either sign,
/// with payloads).
const ENTRIES: [u32; 30] = [
    0x0000_0000,
    0x8000_0000,
    0x3f80_0000,
    0xbf80_0000,
    0x4040_0000, // 3
    0xc0e0_0000, // -7
    0x3f00_0000, // 0.5
    0xbf00_0000, // -0.5
    0x4040_0001, // just above 3
    0x403f_ffff, // just below 3
    0x0000_0001,
    0x8000_0001,
    0x4b00_0000, // 2^23
    0x4aff_ffff, // just below 2^23
    0x4b80_0000, // 2^24
    0x7f7f_ffff,
    0x7f80_0000,
    0xff80_0000,
    0x7fc0_0000,
    0xffc0_0000,
    0x7fc0_0001,
    0xffc1_2345,
    0x7fa0_0000,
    0xffa0_0000,
    0x7f80_0001,
    0xff80_0001,
    0x3f80_0001,
    0x3f7f_ffff,
    0x42c8_8000, // 100.25
    0xcb00_0001,
];

#[test]
fn texcoords_entries_match_oracle() {
    // Each entry set to each value, the rest the identity.
    for slot in 0..16 {
        for &v in &ENTRIES {
            let mut bits = IDENTITY;
            bits[slot] = v;
            check_centered(&bits);
        }
    }
    // Every pair as the translation.
    for &tx in &ENTRIES {
        for &ty in &ENTRIES {
            let mut bits = IDENTITY;
            bits[12] = tx;
            bits[13] = ty;
            check_centered(&bits);
        }
    }
}

// ---------------------------------------------------------------------------
// Proptests

fn any_scale() -> impl Strategy<Value = f32> {
    prop_oneof![
        3 => any::<u32>().prop_map(f32::from_bits),
        1 => (0x3eff_fff0u32..=0x3f00_0010).prop_map(f32::from_bits),
        1 => (0x3f7f_f960u32..=0x3f7f_f980).prop_map(f32::from_bits),
        1 => (0x3f7f_fff0u32..=0x3f80_0010).prop_map(f32::from_bits),
        1 => (0x3f80_0330u32..=0x3f80_0360).prop_map(f32::from_bits),
        1 => prop::sample::select(vec![0x7fc0_0000u32, 0xffc0_0000, 0x7fa0_0000, 0x8000_0000, 0x7f80_0000, 0xff80_0000]).prop_map(f32::from_bits),
    ]
}

fn any_i32_biased() -> impl Strategy<Value = i32> {
    prop_oneof![
        2 => Just(0),
        1 => -2i32..10,
        1 => any::<i32>(),
    ]
}

/// One matrix entry: the identity's value at `slot` most often, else a
/// whole or fractional number, or any bits.
fn entry(slot: usize) -> impl Strategy<Value = u32> {
    let identity = IDENTITY[slot];
    prop_oneof![
        6 => Just(identity),
        1 => any::<i32>().prop_map(|i| {
            #[allow(clippy::cast_precision_loss)]
            let f = i as f32;
            f.to_bits()
        }),
        1 => (any::<i16>(), 1u32..1024).prop_map(|(i, frac)| {
            (f32::from(i) + f32::from(u16::try_from(frac).unwrap_or(1)) / 1024.0).to_bits()
        }),
        1 => any::<u32>(),
        1 => prop::sample::select(ENTRIES.to_vec()),
    ]
}

fn any_matrix() -> impl Strategy<Value = [u32; 16]> {
    let e: [_; 16] = std::array::from_fn(entry);
    e
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn random_triggers_match_oracle(
        c in [any_i32_biased(), any_i32_biased(), any_i32_biased(), any_i32_biased()],
        b in [any_i32_biased(), any_i32_biased(), any_i32_biased(), any_i32_biased()],
        scale_filter in any_i32_biased(),
        blend_method in any_i32_biased(),
        blend_type in any_i32_biased(),
        is_scene in any::<bool>(),
        is_group in any::<bool>(),
    ) {
        check_triggers(&obs_scene_render_triggers {
            crop_left: c[0],
            crop_top: c[1],
            crop_right: c[2],
            crop_bottom: c[3],
            bounds_crop_left: b[0],
            bounds_crop_top: b[1],
            bounds_crop_right: b[2],
            bounds_crop_bottom: b[3],
            scale_filter,
            blend_method,
            blend_type,
            is_scene,
            is_group,
        });
    }

    #[test]
    fn random_effects_match_oracle(
        filter in prop_oneof![-2i32..9, any::<i32>()],
        sx in any_scale(),
        sy in any_scale(),
    ) {
        check_effect(filter, sx, sy);
    }

    #[test]
    fn random_multipliers_match_oracle(
        current in prop_oneof![-1i32..5, any::<i32>()],
        source in prop_oneof![-1i32..5, any::<i32>()],
        white in any::<u32>(),
        upscale in any::<bool>(),
    ) {
        check_multiplier(current, source, f32::from_bits(white));
        check_technique(upscale, current, source);
    }

    #[test]
    fn random_matrices_match_oracle(bits in any_matrix()) {
        check_centered(&bits);
    }
}
