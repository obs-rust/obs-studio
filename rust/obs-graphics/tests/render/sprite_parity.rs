//! Tier 3: the `gs_sprite_*` and `gs_texture_*` plan shims against the
//! original C (`obs-c-oracle`, `oracle/graphics_sprite.c`: the unmodified
//! `libobs/graphics/graphics-sprite.c.h` with its functions renamed).
//!
//! Every float is compared by its bits, NaN included: both sides run the
//! same IEEE operation on the same host. `gs_sprite_build` is compared over
//! every byte of the two arrays it writes, from the same random fill. The
//! plans are compared field by field, the enums as the `int` the C returns.
//!
//! Runs:
//! - the header's constants, the three structs' sizes and offsets and the
//!   enums' sizes against the Rust mirrors;
//! - every flip of the low 16 bits, alone and with the high bits all set,
//!   bit 31 or a pattern, for the normalized, texel and subregion
//!   coordinates and the flipped-buffer rule;
//! - every flag set of the low 9 bits (every flag `graphics.h` defines
//!   and two more), alone and with high bits, over levels and sizes at the
//!   power-of-two edges, for the mipmap plan; and `is_pow2` over every
//!   shift +-8;
//! - the refusals and the texture-size rule over every pair of special
//!   floats, both texture flags, and a strided sweep of every 4093rd `f32`
//!   bit pattern;
//! - every upload of line sizes 0 to 40 and heights 0 to 8, both flips, and
//!   the line sizes and heights at the `u32` and `usize` edges;
//! - proptests (20000 cases each) over sizes, positions and flips drawn
//!   from `u32` bit patterns, floats drawn from `f32` bit patterns (never an
//!   `f32` range), full-range line sizes and heights, and any flags and
//!   levels.
//!
//! Intentional difference (not compared): a null `points` or `uvs` is not
//! written through by the shim; the C writes through it.

use core::ffi::c_int;
use core::mem::{offset_of, size_of};

use obs_c_oracle::graphics_math::OracleVec3;
use obs_c_oracle::graphics_sprite::{self as c, OracleSpriteUv};
use obs_c_oracle::vec2::OracleVec2;
use obs_graphics::ffi::sprite::{self as r, gs_sprite_uv};
use obs_graphics::ffi::vec2::vec2;
use obs_graphics::ffi::vec3::vec3;
use obs_graphics::sprite::{
    BUILD_MIPMAPS, FLIP_U, FLIP_V, MipWarning, QuadRefusal, RENDER_TARGET, UploadMode,
};
use proptest::prelude::*;

fn uv_bits(uv: gs_sprite_uv) -> [u32; 4] {
    [
        uv.start_u.to_bits(),
        uv.end_u.to_bits(),
        uv.start_v.to_bits(),
        uv.end_v.to_bits(),
    ]
}

fn oracle_uv_bits(uv: OracleSpriteUv) -> [u32; 4] {
    [
        uv.start_u.to_bits(),
        uv.end_u.to_bits(),
        uv.start_v.to_bits(),
        uv.end_v.to_bits(),
    ]
}

#[track_caller]
fn check_norm(flip: u32) {
    // SAFETY: plain values in and out.
    let want = oracle_uv_bits(unsafe { c::oracle_gs_sprite_uv_norm(flip) });
    assert_eq!(uv_bits(r::gs_sprite_uv_norm(flip)), want, "norm {flip:#x}");
}

#[track_caller]
fn check_rect(width: u32, height: u32, flip: u32) {
    // SAFETY: plain values in and out.
    let want = oracle_uv_bits(unsafe { c::oracle_gs_sprite_uv_rect(width, height, flip) });
    assert_eq!(
        uv_bits(r::gs_sprite_uv_rect(width, height, flip)),
        want,
        "rect {width}x{height} {flip:#x}"
    );
}

#[track_caller]
fn check_subregion(region: [u32; 6], flip: u32) {
    let [x, y, cx, cy, tex_cx, tex_cy] = region;
    // SAFETY: plain values in and out.
    let want = oracle_uv_bits(unsafe {
        c::oracle_gs_sprite_uv_subregion(x, y, cx, cy, tex_cx, tex_cy, flip)
    });
    assert_eq!(
        uv_bits(r::gs_sprite_uv_subregion(
            x, y, cx, cy, tex_cx, tex_cy, flip
        )),
        want,
        "subregion {region:?} {flip:#x}"
    );
    // SAFETY: plain values in and out.
    let whole = unsafe { c::oracle_gs_sprite_subregion_is_whole(x, y, cx, cy, tex_cx, tex_cy) };
    assert_eq!(
        r::gs_sprite_subregion_is_whole(x, y, cx, cy, tex_cx, tex_cy),
        whole,
        "whole {region:?}"
    );
}

#[track_caller]
fn check_flipped_buffer(flip: u32) {
    // SAFETY: plain values in and out.
    let want = unsafe { c::oracle_gs_sprite_quad_flipped_buffer(flip) };
    assert_eq!(r::gs_sprite_quad_flipped_buffer(flip), want, "{flip:#x}");
}

#[track_caller]
fn check_build(cx: f32, cy: f32, uv: [f32; 4], fill: u8) {
    let mut c_points = [OracleVec3::default(); 4];
    let mut c_uvs = [OracleVec2::default(); 4];
    let mut r_points = [vec3::default(); 4];
    let mut r_uvs = [vec2::default(); 4];
    let bits = |f: u8| f32::from_bits(u32::from_ne_bytes([f; 4]));
    for p in &mut c_points {
        *p = OracleVec3 {
            x: bits(fill),
            y: bits(fill),
            z: bits(fill),
            w: bits(fill),
        };
    }
    for p in &mut r_points {
        *p = vec3 {
            x: bits(fill),
            y: bits(fill),
            z: bits(fill),
            w: bits(fill),
        };
    }
    for v in &mut c_uvs {
        *v = OracleVec2 {
            x: bits(fill),
            y: bits(fill),
        };
    }
    for v in &mut r_uvs {
        *v = vec2 {
            x: bits(fill),
            y: bits(fill),
        };
    }
    let [start_u, end_u, start_v, end_v] = uv;
    // SAFETY: four writable vec3 and vec2 each.
    unsafe {
        c::oracle_gs_sprite_build(
            c_points.as_mut_ptr(),
            c_uvs.as_mut_ptr(),
            cx,
            cy,
            OracleSpriteUv {
                start_u,
                end_u,
                start_v,
                end_v,
            },
        );
        r::gs_sprite_build(
            r_points.as_mut_ptr(),
            r_uvs.as_mut_ptr(),
            cx,
            cy,
            gs_sprite_uv {
                start_u,
                end_u,
                start_v,
                end_v,
            },
        );
    }
    let c_point_bits: Vec<[u32; 4]> = c_points
        .iter()
        .map(|p| [p.x.to_bits(), p.y.to_bits(), p.z.to_bits(), p.w.to_bits()])
        .collect();
    let r_point_bits: Vec<[u32; 4]> = r_points
        .iter()
        .map(|p| [p.x.to_bits(), p.y.to_bits(), p.z.to_bits(), p.w.to_bits()])
        .collect();
    assert_eq!(r_point_bits, c_point_bits, "points of {cx:e}x{cy:e}");
    let c_uv_bits: Vec<[u32; 2]> = c_uvs
        .iter()
        .map(|v| [v.x.to_bits(), v.y.to_bits()])
        .collect();
    let r_uv_bits: Vec<[u32; 2]> = r_uvs
        .iter()
        .map(|v| [v.x.to_bits(), v.y.to_bits()])
        .collect();
    assert_eq!(r_uv_bits, c_uv_bits, "uvs of {uv:?}");
}

#[track_caller]
fn check_quad(has_texture: bool, texture_2d: bool, width: f32, height: f32) {
    // SAFETY: plain values in and out.
    let want = unsafe { c::oracle_gs_sprite_quad_refusal(has_texture, texture_2d, width, height) };
    assert_eq!(
        r::gs_sprite_quad_refusal(has_texture, texture_2d, width, height),
        want,
        "{has_texture} {texture_2d} {width:e} {height:e}"
    );
    // SAFETY: plain values in and out.
    let want = unsafe { c::oracle_gs_sprite_quad_texture_size(width) };
    assert_eq!(r::gs_sprite_quad_texture_size(width), want, "{width:e}");
}

#[track_caller]
fn check_upload(linesize: u32, linesize_out: u32, height: usize, flip: bool) {
    // SAFETY: plain values in and out.
    let want = unsafe { c::oracle_gs_texture_upload_plan(linesize, linesize_out, height, flip) };
    let got = r::gs_texture_upload_plan(linesize, linesize_out, height, flip);
    assert_eq!(
        (got.row_copy, got.whole_size, got.src_offset, got.mode),
        (want.row_copy, want.whole_size, want.src_offset, want.mode),
        "upload {linesize} {linesize_out} {height} {flip}"
    );
}

#[track_caller]
fn check_mip(width: u32, height: u32, flags: u32, levels: u32) {
    // SAFETY: plain values in and out.
    let want = unsafe { c::oracle_gs_texture_mip_plan(width, height, flags, levels) };
    let got = r::gs_texture_mip_plan(width, height, flags, levels);
    assert_eq!(
        (got.flags, got.levels, got.warning),
        (want.flags, want.levels, want.warning),
        "mip {width}x{height} {flags:#x} {levels}"
    );
}

#[track_caller]
fn check_pow2(size: u32) {
    // SAFETY: plain values in and out.
    let want = unsafe { c::oracle_gs_texture_is_pow2(size) };
    assert_eq!(r::gs_texture_is_pow2(size), want, "{size}");
}

#[test]
fn constants_and_layout() {
    let constants: [i64; 14] = [
        FLIP_U.into(),
        FLIP_V.into(),
        BUILD_MIPMAPS.into(),
        RENDER_TARGET.into(),
        QuadRefusal::Draw as i64,
        QuadRefusal::NotTwoD as i64,
        QuadRefusal::NoSize as i64,
        UploadMode::None as i64,
        UploadMode::Flipped as i64,
        UploadMode::Whole as i64,
        UploadMode::Rows as i64,
        MipWarning::Ok as i64,
        MipWarning::NotPow2 as i64,
        MipWarning::RenderTarget as i64,
    ];
    for (which, (name, value)) in c::CONSTANT_NAMES.iter().zip(constants).enumerate() {
        // SAFETY: an index into the table.
        assert_eq!(
            unsafe { c::oracle_graphics_sprite_constant(which) },
            value,
            "{name}"
        );
    }
    let layout: [usize; 17] = [
        size_of::<gs_sprite_uv>(),
        offset_of!(gs_sprite_uv, start_u),
        offset_of!(gs_sprite_uv, end_u),
        offset_of!(gs_sprite_uv, start_v),
        offset_of!(gs_sprite_uv, end_v),
        size_of::<r::gs_texture_upload_plan>(),
        offset_of!(r::gs_texture_upload_plan, row_copy),
        offset_of!(r::gs_texture_upload_plan, whole_size),
        offset_of!(r::gs_texture_upload_plan, src_offset),
        offset_of!(r::gs_texture_upload_plan, mode),
        size_of::<r::gs_texture_mip_plan>(),
        offset_of!(r::gs_texture_mip_plan, flags),
        offset_of!(r::gs_texture_mip_plan, levels),
        offset_of!(r::gs_texture_mip_plan, warning),
        size_of::<c_int>(),
        size_of::<c_int>(),
        size_of::<c_int>(),
    ];
    for (which, (name, value)) in c::LAYOUT_NAMES.iter().zip(layout).enumerate() {
        // SAFETY: an index into the table.
        assert_eq!(
            unsafe { c::oracle_graphics_sprite_layout(which) },
            value,
            "{name}"
        );
    }
    assert_eq!(size_of::<vec3>(), 16);
    assert_eq!(size_of::<vec2>(), 8);
}

/// Sizes at the float rounding and power-of-two edges.
const SIZES: [u32; 14] = [
    0,
    1,
    2,
    3,
    255,
    256,
    1080,
    1920,
    16_777_216,
    16_777_217,
    16_777_219,
    0x8000_0000,
    0x8000_0001,
    u32::MAX,
];

/// The high halves a flip of the low 16 bits is combined with.
const HIGH: [u32; 4] = [0, 0xffff_0000, 0x8000_0000, 0x5a5a_0000];

#[test]
fn every_flip() {
    let regions: [[u32; 6]; 6] = [
        [480, 270, 960, 540, 1920, 1080],
        [0, 0, 1920, 1080, 1920, 1080],
        [1, 1, 1, 1, 3, 3],
        [0, 0, 10, 20, 0, 0],
        [5, 6, 0, 0, 0, 7],
        [u32::MAX, 16_777_216, u32::MAX, 1, 3, 16_777_217],
    ];
    for high in HIGH {
        for low in 0..=0xffffu32 {
            let flip = high | low;
            check_norm(flip);
            check_flipped_buffer(flip);
            check_rect(1920, 1080, flip);
            check_rect(u32::MAX, 16_777_217, flip);
            if low < 0x100 {
                for region in regions {
                    check_subregion(region, flip);
                }
                for w in SIZES {
                    check_rect(w, 0, flip);
                    check_rect(0, w, flip);
                }
            }
        }
    }
}

#[test]
fn every_subregion_of_small_textures() {
    for tex_cx in 0..=5 {
        for tex_cy in [0, 1, 4] {
            for x in 0..=6 {
                for cx in 0..=6 {
                    for flip in 0..4 {
                        check_subregion([x, 1, cx, tex_cy, tex_cx, tex_cy], flip);
                        check_subregion([0, x, tex_cx, cx, tex_cx, tex_cy], flip);
                        check_subregion([0, 0, cx, tex_cy, tex_cx, tex_cy], flip);
                        check_subregion([0, 0, tex_cx, cx, tex_cx, tex_cy], flip);
                    }
                }
            }
        }
    }
}

#[test]
fn every_flag_set() {
    let levels = [0, 1, 2, 3, 5, 9, 13, 16, 0x8000_0000, u32::MAX];
    let sizes = [
        (256, 256),
        (2, 2),
        (2, 4096),
        (0x8000_0000, 2),
        (1, 1),
        (0, 0),
        (1920, 1080),
        (256, 255),
        (255, 256),
        (1, 256),
        (256, 0),
        (u32::MAX, u32::MAX),
        (3, 4),
    ];
    for (width, height) in sizes {
        for level in levels {
            for low in 0..0x200u32 {
                for high in [0, 0xffff_fe00, 0x8000_0000] {
                    check_mip(width, height, high | low, level);
                }
            }
        }
    }
    for shift in 0..32 {
        let size = 1u32 << shift;
        for delta in 0..=8u32 {
            check_pow2(size.wrapping_add(delta));
            check_pow2(size.wrapping_sub(delta));
        }
    }
    for size in SIZES {
        check_pow2(size);
    }
}

/// Floats at the IEEE edges: zeros, ones, infinities, NaNs (quiet,
/// signalling, negative, with a payload), denormals, the largest finite.
const SPECIAL: [u32; 16] = [
    0x0000_0000,
    0x8000_0000,
    0x3f80_0000,
    0xbf80_0000,
    0x7f80_0000,
    0xff80_0000,
    0x7fc0_0000,
    0xffc0_0000,
    0x7f80_0001,
    0x7fc1_2345,
    0x0000_0001,
    0x8000_0001,
    0x007f_ffff,
    0x7f7f_ffff,
    0x44f0_0000,
    0x4486_e000,
];

#[test]
fn every_special_quad() {
    for w in SPECIAL {
        for h in SPECIAL {
            for has_texture in [false, true] {
                for texture_2d in [false, true] {
                    check_quad(
                        has_texture,
                        texture_2d,
                        f32::from_bits(w),
                        f32::from_bits(h),
                    );
                }
            }
        }
    }
}

#[test]
fn strided_float_sweep() {
    for bits in (0..=u32::MAX).step_by(4093) {
        let f = f32::from_bits(bits);
        check_quad(false, false, f, 1.0);
        check_quad(false, true, 1.0, f);
        check_quad(true, false, f, f);
        check_build(f, -f, [f, 0.0, -f, f], 0xa5);
    }
}

#[test]
fn every_special_build() {
    for a in SPECIAL {
        for b in SPECIAL {
            let (fa, fb) = (f32::from_bits(a), f32::from_bits(b));
            check_build(fa, fb, [fb, fa, fa, fb], 0x5a);
            check_build(fb, fa, [fa, fa, fb, fb], 0xff);
        }
    }
}

#[test]
fn every_small_upload() {
    for linesize in 0..=40u32 {
        for linesize_out in 0..=40u32 {
            for height in 0..=8usize {
                for flip in [false, true] {
                    check_upload(linesize, linesize_out, height, flip);
                }
            }
        }
    }
    let lines = [0, 1, 4, 7680, 8192, 0x8000_0000, u32::MAX - 1, u32::MAX];
    let heights = [
        0,
        1,
        2,
        1080,
        u32::MAX as usize,
        usize::MAX / 2,
        usize::MAX - 1,
        usize::MAX,
    ];
    for linesize in lines {
        for linesize_out in lines {
            for height in heights {
                for flip in [false, true] {
                    check_upload(linesize, linesize_out, height, flip);
                }
            }
        }
    }
}

fn any_u32() -> impl Strategy<Value = u32> {
    prop_oneof![
        any::<u32>(),
        0u32..4096,
        prop::sample::select(SIZES.to_vec()),
    ]
}

fn any_f32() -> impl Strategy<Value = f32> {
    prop_oneof![
        any::<u32>().prop_map(f32::from_bits),
        prop::sample::select(SPECIAL.to_vec()).prop_map(f32::from_bits),
    ]
}

fn any_usize() -> impl Strategy<Value = usize> {
    prop_oneof![any::<usize>(), 0usize..4096]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20000))]

    #[test]
    fn proptest_coordinates(flip in any::<u32>(), x in any_u32(), y in any_u32(), cx in any_u32(),
                            cy in any_u32(), tex_cx in any_u32(), tex_cy in any_u32()) {
        check_norm(flip);
        check_rect(tex_cx, tex_cy, flip);
        check_subregion([x, y, cx, cy, tex_cx, tex_cy], flip);
        check_subregion([0, 0, cx, cy, cx, cy], flip);
        check_flipped_buffer(flip);
    }

    #[test]
    fn proptest_build(cx in any_f32(), cy in any_f32(), a in any_f32(), b in any_f32(), c in any_f32(),
                      d in any_f32(), fill in any::<u8>()) {
        check_build(cx, cy, [a, b, c, d], fill);
    }

    #[test]
    fn proptest_quad(has_texture in any::<bool>(), texture_2d in any::<bool>(), w in any_f32(), h in any_f32()) {
        check_quad(has_texture, texture_2d, w, h);
    }

    #[test]
    fn proptest_upload(linesize in any_u32(), linesize_out in any_u32(), height in any_usize(),
                       flip in any::<bool>(), equal in any::<bool>()) {
        check_upload(linesize, if equal { linesize } else { linesize_out }, height, flip);
    }

    #[test]
    fn proptest_mip(width in any_u32(), height in any_u32(), flags in any::<u32>(), levels in any_u32(),
                    pow2 in any::<bool>(), wshift in 0u32..32, hshift in 0u32..32) {
        if pow2 {
            check_mip(1 << wshift, 1 << hshift, flags, levels);
        } else {
            check_mip(width, height, flags, levels);
        }
        check_pow2(width);
    }
}
