//! Tier 1: `obs_graphics::sprite`, the sprite and texture upload geometry
//! of `graphics.c`.
//!
//! The `steps_*` cases port `test/cmocka/test_sprite_geometry_steps.c` 1:1,
//! under the same case names, in the same order, with the same inputs and
//! expectations. Floats are compared by their bits; an expected NaN is only
//! asked to be a NaN (which NaN `0 / 0` gives is the host's).
//!
//! The `edge_*` cases are new. They check the rules from their other side:
//! a flip swaps exactly its own axis and ignores every other bit, for every
//! flip value of a bit-pattern proptest; the rectangle coordinates are the
//! normalized ones times the size; a subregion of the whole texture is the
//! normalized sprite; the points never depend on the coordinates; the
//! upload plan's bytes per row are the shorter line, and its offsets are
//! `usize` arithmetic checked against `u128`; [`is_pow2`] against a count
//! of the bits for every shift and a proptest; and the mipmap plan is
//! stable (its own output plans no change), keeps every flag but
//! [`BUILD_MIPMAPS`], and warns exactly when it changes something, over
//! every flag set and a proptest of sizes and levels.

use obs_graphics::sprite::{
    BUILD_MIPMAPS, FLIP_U, FLIP_V, MipPlan, MipWarning, QuadRefusal, RENDER_TARGET, SpriteUv,
    UploadMode, UploadPlan, is_pow2, quad_flipped_buffer, quad_refusal, quad_texture_size, rect_uv,
    sprite, sprite_uv, subregion_is_whole, subsprite_uv, texture_flags, upload_plan,
};
use obs_graphics::vec2::Vec2;
use obs_graphics::vec3::Vec3;
use proptest::prelude::*;

/// `GS_DYNAMIC` and `GS_SHARED_TEX` (`graphics.h`): flags the mipmap rules
/// keep.
const OTHER_FLAGS: u32 = (1 << 1) | (1 << 5);

/// Every flip value of the C test: none, U, V, both, and with other bits.
const FLIPS: [u32; 10] = [
    0,
    FLIP_U,
    FLIP_V,
    FLIP_U | FLIP_V,
    1 << 2,
    FLIP_U | (1 << 2),
    FLIP_V | (1 << 31),
    FLIP_U | FLIP_V | 0xffff_fff0,
    0xffff_fffc,
    u32::MAX,
];

#[track_caller]
fn check_float(actual: f32, expected: f32) {
    if expected.is_nan() {
        assert!(actual.is_nan(), "{actual:e} is not a NaN");
    } else {
        assert_eq!(
            actual.to_bits(),
            expected.to_bits(),
            "{actual:e} ({:#010x}), expected {expected:e} ({:#010x})",
            actual.to_bits(),
            expected.to_bits()
        );
    }
}

#[track_caller]
fn check_uv(uv: SpriteUv, su: f32, eu: f32, sv: f32, ev: f32) {
    check_float(uv.start_u, su);
    check_float(uv.end_u, eu);
    check_float(uv.start_v, sv);
    check_float(uv.end_v, ev);
}

const fn has_u(flip: u32) -> bool {
    flip & FLIP_U != 0
}

const fn has_v(flip: u32) -> bool {
    flip & FLIP_V != 0
}

const fn uv(start_u: f32, end_u: f32, start_v: f32, end_v: f32) -> SpriteUv {
    SpriteUv {
        start_u,
        end_u,
        start_v,
        end_v,
    }
}

#[track_caller]
fn check_points(points: [Vec3; 4], expected: [[f32; 4]; 4]) {
    for (point, want) in points.iter().zip(expected) {
        check_float(point.x, want[0]);
        check_float(point.y, want[1]);
        check_float(point.z, want[2]);
        check_float(point.w, want[3]);
    }
}

#[track_caller]
fn check_uvs(uvs: [Vec2; 4], expected: [[f32; 2]; 4]) {
    for (got, want) in uvs.iter().zip(expected) {
        check_float(got.x, want[0]);
        check_float(got.y, want[1]);
    }
}

#[test]
fn steps_build_writes_every_lane() {
    let (points, uvs) = sprite(1920.0, 1080.0, uv(0.25, 0.75, 0.125, 0.875));
    check_points(
        points,
        [
            [0.0, 0.0, 0.0, 0.0],
            [1920.0, 0.0, 0.0, 0.0],
            [0.0, 1080.0, 0.0, 0.0],
            [1920.0, 1080.0, 0.0, 0.0],
        ],
    );
    check_uvs(
        uvs,
        [[0.25, 0.125], [0.75, 0.125], [0.25, 0.875], [0.75, 0.875]],
    );
}

#[test]
fn steps_build_keeps_bits() {
    let nan_payload = f32::from_bits(0x7fc1_2345);
    let denormal = f32::from_bits(1);
    let (points, uvs) = sprite(
        -0.0,
        f32::NEG_INFINITY,
        uv(-0.0, f32::INFINITY, nan_payload, denormal),
    );
    let bits = |p: Vec3| [p.x.to_bits(), p.y.to_bits(), p.z.to_bits(), p.w.to_bits()];
    let neg0 = (-0.0f32).to_bits();
    let ninf = f32::NEG_INFINITY.to_bits();
    assert_eq!(bits(points[0]), [0, 0, 0, 0]);
    assert_eq!(bits(points[1]), [neg0, 0, 0, 0]);
    assert_eq!(bits(points[2]), [0, ninf, 0, 0]);
    assert_eq!(bits(points[3]), [neg0, ninf, 0, 0]);
    let uv_bits: Vec<[u32; 2]> = uvs.iter().map(|v| [v.x.to_bits(), v.y.to_bits()]).collect();
    let inf = f32::INFINITY.to_bits();
    assert_eq!(
        uv_bits,
        [[neg0, 0x7fc1_2345], [inf, 0x7fc1_2345], [neg0, 1], [inf, 1]]
    );
}

#[test]
fn steps_uv_norm_each_flip() {
    for flip in FLIPS {
        let (su, eu) = if has_u(flip) { (1.0, 0.0) } else { (0.0, 1.0) };
        let (sv, ev) = if has_v(flip) { (1.0, 0.0) } else { (0.0, 1.0) };
        check_uv(sprite_uv(flip), su, eu, sv, ev);
    }
    check_uv(sprite_uv(0), 0.0, 1.0, 0.0, 1.0);
    check_uv(sprite_uv(FLIP_U), 1.0, 0.0, 0.0, 1.0);
    check_uv(sprite_uv(FLIP_V), 0.0, 1.0, 1.0, 0.0);
    check_uv(sprite_uv(FLIP_U | FLIP_V), 1.0, 0.0, 1.0, 0.0);
    check_uv(sprite_uv(1 << 2), 0.0, 1.0, 0.0, 1.0);
}

#[test]
fn steps_static_sprite_buffers() {
    let plain_points = [
        [0.0, 0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0, 0.0],
    ];
    let (points, uvs) = sprite(1.0, 1.0, sprite_uv(0));
    check_points(points, plain_points);
    check_uvs(uvs, [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]]);
    let (points, uvs) = sprite(1.0, 1.0, sprite_uv(FLIP_V));
    check_points(points, plain_points);
    check_uvs(uvs, [[0.0, 1.0], [1.0, 1.0], [0.0, 0.0], [1.0, 0.0]]);
}

#[test]
fn steps_uv_rect_each_flip() {
    let sizes: [(u32, u32, f32, f32); 5] = [
        (1920, 1080, 1920.0, 1080.0),
        (0, 0, 0.0, 0.0),
        (1, 0, 1.0, 0.0),
        (16_777_217, 16_777_219, 16_777_216.0, 16_777_220.0),
        (u32::MAX, 0x8000_0001, 4_294_967_296.0, 2_147_483_648.0),
    ];
    for (width, height, w, h) in sizes {
        for flip in FLIPS {
            let (su, eu) = if has_u(flip) { (w, 0.0) } else { (0.0, w) };
            let (sv, ev) = if has_v(flip) { (h, 0.0) } else { (0.0, h) };
            check_uv(rect_uv(width, height, flip), su, eu, sv, ev);
        }
    }
}

#[test]
fn steps_uv_subregion_inside() {
    for flip in FLIPS {
        let (su, eu) = if has_u(flip) {
            (0.75, 0.25)
        } else {
            (0.25, 0.75)
        };
        let (sv, ev) = if has_v(flip) {
            (0.75, 0.25)
        } else {
            (0.25, 0.75)
        };
        check_uv(
            subsprite_uv(480, 270, 960, 540, 1920, 1080, flip),
            su,
            eu,
            sv,
            ev,
        );
    }
    let third = subsprite_uv(1, 1, 1, 1, 3, 3, 0);
    check_uv(third, 1.0 / 3.0, 2.0 / 3.0, 1.0 / 3.0, 2.0 / 3.0);
    assert_eq!(third.start_u.to_bits(), 0x3eaa_aaab);
    assert_eq!(third.end_u.to_bits(), 0x3f2a_aaab);
}

#[test]
fn steps_uv_subregion_edges() {
    check_uv(
        subsprite_uv(0, 0, 1920, 1080, 1920, 1080, 0),
        0.0,
        1.0,
        0.0,
        1.0,
    );
    check_uv(
        subsprite_uv(0, 0, 1920, 1080, 1920, 1080, FLIP_U | FLIP_V),
        1.0,
        0.0,
        1.0,
        0.0,
    );
    check_uv(subsprite_uv(0, 0, 1, 1, 4, 2, 0), 0.0, 0.25, 0.0, 0.5);
    check_uv(subsprite_uv(3, 1, 1, 1, 4, 2, 0), 0.75, 1.0, 0.5, 1.0);
    check_uv(subsprite_uv(3, 1, 1, 1, 4, 2, FLIP_U), 1.0, 0.75, 0.5, 1.0);
    check_uv(subsprite_uv(3, 1, 1, 1, 4, 2, FLIP_V), 0.75, 1.0, 1.0, 0.5);
    check_uv(subsprite_uv(4, 2, 0, 0, 4, 2, 0), 1.0, 1.0, 1.0, 1.0);
    check_uv(
        subsprite_uv(0, 0, 0, 0, 4, 2, FLIP_U | FLIP_V),
        0.0,
        0.0,
        0.0,
        0.0,
    );
}

#[test]
fn steps_uv_subregion_larger_than_texture() {
    check_uv(
        subsprite_uv(0, 0, 3840, 2160, 1920, 1080, 0),
        0.0,
        2.0,
        0.0,
        2.0,
    );
    check_uv(
        subsprite_uv(1920, 1080, 960, 540, 1920, 1080, 0),
        1.0,
        1.5,
        1.0,
        1.5,
    );
    check_uv(
        subsprite_uv(1920, 1080, 960, 540, 1920, 1080, FLIP_U | FLIP_V),
        1.5,
        1.0,
        1.5,
        1.0,
    );
    check_uv(
        subsprite_uv(u32::MAX, 0, u32::MAX, u32::MAX, 1, 1, 0),
        4_294_967_296.0,
        8_589_934_592.0,
        0.0,
        4_294_967_296.0,
    );
    check_uv(
        subsprite_uv(16_777_216, 0, 1, 1, 16_777_216, 1, 0),
        1.0,
        1.0,
        0.0,
        1.0,
    );
}

#[test]
fn steps_uv_subregion_zero_size_texture() {
    let inf = f32::INFINITY;
    let nan = f32::NAN;
    check_uv(subsprite_uv(0, 0, 0, 0, 0, 0, 0), nan, nan, nan, nan);
    check_uv(subsprite_uv(0, 0, 10, 20, 0, 0, 0), nan, inf, nan, inf);
    check_uv(
        subsprite_uv(0, 0, 10, 20, 0, 0, FLIP_U | FLIP_V),
        inf,
        nan,
        inf,
        nan,
    );
    check_uv(subsprite_uv(5, 6, 10, 20, 0, 0, FLIP_U), inf, inf, inf, inf);
    check_uv(subsprite_uv(1, 0, 1, 1, 2, 0, 0), 0.5, 1.0, nan, inf);
    check_uv(subsprite_uv(0, 1, 1, 1, 0, 2, FLIP_V), nan, inf, 1.0, 0.5);
}

#[test]
fn steps_subregion_whole_shortcut() {
    assert!(subregion_is_whole(0, 0, 1920, 1080, 1920, 1080));
    assert!(!subregion_is_whole(1, 0, 1920, 1080, 1920, 1080));
    assert!(!subregion_is_whole(0, 1, 1920, 1080, 1920, 1080));
    assert!(!subregion_is_whole(0, 0, 1919, 1080, 1920, 1080));
    assert!(!subregion_is_whole(0, 0, 1920, 1079, 1920, 1080));
    assert!(!subregion_is_whole(0, 0, 1921, 1080, 1920, 1080));
    assert!(!subregion_is_whole(0, 0, 1080, 1920, 1920, 1080));
    assert!(subregion_is_whole(
        0,
        0,
        u32::MAX,
        u32::MAX,
        u32::MAX,
        u32::MAX
    ));
    assert!(subregion_is_whole(0, 0, 0, 0, 0, 0));
    assert!(!subregion_is_whole(0, 0, 1, 0, 0, 0));
}

#[test]
fn steps_quad_refusals() {
    let sizes = [
        0.0,
        -0.0,
        1.0,
        -1.0,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NAN,
        1e-45,
        1920.0,
    ];
    for width in sizes {
        for height in sizes {
            let zero = width == 0.0 || height == 0.0;
            let none = if zero {
                QuadRefusal::NoSize
            } else {
                QuadRefusal::Draw
            };
            assert_eq!(quad_refusal(true, true, width, height), QuadRefusal::Draw);
            assert_eq!(
                quad_refusal(true, false, width, height),
                QuadRefusal::NotTwoD
            );
            assert_eq!(quad_refusal(false, true, width, height), none);
            assert_eq!(quad_refusal(false, false, width, height), none);
        }
    }
    assert_eq!(quad_refusal(false, false, 0.0, 1.0), QuadRefusal::NoSize);
    assert_eq!(quad_refusal(false, false, 1.0, -0.0), QuadRefusal::NoSize);
    assert_eq!(
        quad_refusal(false, false, f32::NAN, -1.0),
        QuadRefusal::Draw
    );
}

#[test]
fn steps_quad_texture_size() {
    assert!(quad_texture_size(0.0));
    assert!(quad_texture_size(-0.0));
    assert!(!quad_texture_size(1e-45));
    assert!(!quad_texture_size(-1e-45));
    assert!(!quad_texture_size(1.0));
    assert!(!quad_texture_size(-1.0));
    assert!(!quad_texture_size(f32::INFINITY));
    assert!(!quad_texture_size(f32::NAN));
    assert!(!quad_texture_size(f32::from_bits(0xffc0_0000)));
}

#[test]
fn steps_quad_flipped_buffer() {
    assert!(!quad_flipped_buffer(0));
    for flip in &FLIPS[1..] {
        assert!(quad_flipped_buffer(*flip));
    }
    assert!(quad_flipped_buffer(FLIP_U));
    assert!(quad_flipped_buffer(1 << 2));
}

#[track_caller]
fn check_plan(
    plan: UploadPlan,
    mode: UploadMode,
    row_copy: usize,
    whole_size: usize,
    src_offset: usize,
) {
    assert_eq!(
        plan,
        UploadPlan {
            row_copy,
            whole_size,
            src_offset,
            mode,
        }
    );
}

const U32: usize = u32::MAX as usize;

#[test]
fn steps_upload_equal_linesizes() {
    check_plan(
        upload_plan(7680, 7680, 1080, false),
        UploadMode::Whole,
        7680,
        7680 * 1080,
        0,
    );
    check_plan(upload_plan(4, 4, 1, false), UploadMode::Whole, 4, 4, 0);
    check_plan(
        upload_plan(7680, 7680, 0, false),
        UploadMode::Whole,
        7680,
        0,
        0,
    );
    check_plan(upload_plan(0, 0, 1080, false), UploadMode::Whole, 0, 0, 0);
    check_plan(
        upload_plan(u32::MAX, u32::MAX, U32, false),
        UploadMode::Whole,
        U32,
        U32.wrapping_mul(U32),
        0,
    );
}

#[test]
fn steps_upload_unequal_linesizes() {
    check_plan(
        upload_plan(7680, 8192, 1080, false),
        UploadMode::Rows,
        7680,
        0,
        0,
    );
    check_plan(
        upload_plan(8192, 7680, 1080, false),
        UploadMode::Rows,
        7680,
        0,
        0,
    );
    check_plan(upload_plan(0, 64, 4, false), UploadMode::Rows, 0, 0, 0);
    check_plan(upload_plan(64, 0, 4, false), UploadMode::Rows, 0, 0, 0);
    check_plan(
        upload_plan(1, u32::MAX, 2, false),
        UploadMode::Rows,
        1,
        0,
        0,
    );
    check_plan(
        upload_plan(7680, 8192, 0, false),
        UploadMode::Rows,
        7680,
        0,
        0,
    );
}

#[test]
fn steps_upload_flipped() {
    check_plan(
        upload_plan(7680, 7680, 1080, true),
        UploadMode::Flipped,
        7680,
        0,
        1079 * 7680,
    );
    check_plan(
        upload_plan(7680, 8192, 1080, true),
        UploadMode::Flipped,
        7680,
        0,
        1079 * 7680,
    );
    check_plan(
        upload_plan(8192, 7680, 1080, true),
        UploadMode::Flipped,
        7680,
        0,
        1079 * 8192,
    );
    check_plan(upload_plan(16, 16, 1, true), UploadMode::Flipped, 16, 0, 0);
    check_plan(upload_plan(0, 16, 3, true), UploadMode::Flipped, 0, 0, 0);
    check_plan(
        upload_plan(u32::MAX, 1, U32, true),
        UploadMode::Flipped,
        1,
        0,
        (U32 - 1).wrapping_mul(U32),
    );
}

#[test]
fn steps_upload_flipped_height_zero() {
    check_plan(
        upload_plan(7680, 7680, 0, true),
        UploadMode::None,
        7680,
        0,
        0,
    );
    check_plan(
        upload_plan(7680, 8192, 0, true),
        UploadMode::None,
        7680,
        0,
        0,
    );
    check_plan(upload_plan(0, 0, 0, true), UploadMode::None, 0, 0, 0);
    check_plan(
        upload_plan(u32::MAX, u32::MAX, 0, true),
        UploadMode::None,
        U32,
        0,
        0,
    );
}

#[test]
fn steps_pow2() {
    assert!(!is_pow2(0));
    assert!(!is_pow2(1));
    for shift in 1..32 {
        let size = 1u32 << shift;
        assert!(is_pow2(size));
        assert!(!is_pow2(size + 1));
        if size > 2 {
            assert!(!is_pow2(size - 1));
        }
        assert!(!is_pow2(size | 1));
    }
    assert!(!is_pow2(3));
    assert!(!is_pow2(1080));
    assert!(!is_pow2(0x8000_0001));
    assert!(!is_pow2(u32::MAX));
}

/// The ten sizes of the C test, with whether both axes are powers of two.
const MIP_SIZES: [(u32, u32, bool); 10] = [
    (256, 256, true),
    (2, 4096, true),
    (1 << 31, 2, true),
    (1920, 1080, false),
    (256, 255, false),
    (255, 256, false),
    (1, 1, false),
    (0, 0, false),
    (1, 256, false),
    (256, 0, false),
];

/// The flag sets of the C test, with and without the other flags.
fn flag_sets() -> impl Iterator<Item = u32> {
    [
        0,
        BUILD_MIPMAPS,
        RENDER_TARGET,
        BUILD_MIPMAPS | RENDER_TARGET,
    ]
    .into_iter()
    .flat_map(|f| [f, f | OTHER_FLAGS])
}

#[test]
fn steps_mip_rules() {
    for (width, height, pow2) in MIP_SIZES {
        for levels in [0, 1, 5] {
            for flags in flag_sets() {
                let mipmaps = flags & BUILD_MIPMAPS != 0 || levels != 1;
                let warning = if mipmaps && !pow2 {
                    MipWarning::NotPow2
                } else if mipmaps && flags & RENDER_TARGET != 0 {
                    MipWarning::RenderTarget
                } else {
                    MipWarning::Ok
                };
                let want = if warning == MipWarning::Ok {
                    MipPlan {
                        flags,
                        levels,
                        warning,
                    }
                } else {
                    MipPlan {
                        flags: flags & !BUILD_MIPMAPS,
                        levels: 1,
                        warning,
                    }
                };
                assert_eq!(
                    texture_flags(width, height, flags, levels),
                    want,
                    "{width}x{height} flags {flags:#x} levels {levels}"
                );
            }
        }
    }
}

#[track_caller]
fn check_mip(plan: MipPlan, warning: MipWarning, flags: u32, levels: u32) {
    assert_eq!(
        plan,
        MipPlan {
            flags,
            levels,
            warning,
        }
    );
}

#[test]
fn steps_mip_rule_rows() {
    check_mip(texture_flags(256, 256, 0, 1), MipWarning::Ok, 0, 1);
    check_mip(texture_flags(256, 256, 0, 0), MipWarning::Ok, 0, 0);
    check_mip(
        texture_flags(256, 256, BUILD_MIPMAPS, 5),
        MipWarning::Ok,
        BUILD_MIPMAPS,
        5,
    );
    check_mip(
        texture_flags(256, 256, BUILD_MIPMAPS, 1),
        MipWarning::Ok,
        BUILD_MIPMAPS,
        1,
    );
    check_mip(
        texture_flags(256, 256, RENDER_TARGET, 1),
        MipWarning::Ok,
        RENDER_TARGET,
        1,
    );
    check_mip(
        texture_flags(256, 256, RENDER_TARGET, 0),
        MipWarning::RenderTarget,
        RENDER_TARGET,
        1,
    );
    check_mip(
        texture_flags(256, 256, RENDER_TARGET | BUILD_MIPMAPS | OTHER_FLAGS, 5),
        MipWarning::RenderTarget,
        RENDER_TARGET | OTHER_FLAGS,
        1,
    );
    check_mip(
        texture_flags(1920, 1080, RENDER_TARGET | BUILD_MIPMAPS, 5),
        MipWarning::NotPow2,
        RENDER_TARGET,
        1,
    );
    check_mip(texture_flags(1920, 1080, 0, 0), MipWarning::NotPow2, 0, 1);
    check_mip(
        texture_flags(1920, 1080, RENDER_TARGET, 1),
        MipWarning::Ok,
        RENDER_TARGET,
        1,
    );
    check_mip(texture_flags(1, 1, 0, 0), MipWarning::NotPow2, 0, 1);
    check_mip(
        texture_flags(256, 255, BUILD_MIPMAPS, 1),
        MipWarning::NotPow2,
        0,
        1,
    );
    check_mip(
        texture_flags(300, 300, u32::MAX, 9),
        MipWarning::NotPow2,
        0xffff_fffe,
        1,
    );
}

/// `uv` with u swapped when `u` and v swapped when `v`.
const fn swapped(base: SpriteUv, u: bool, v: bool) -> SpriteUv {
    let (start_u, end_u) = if u {
        (base.end_u, base.start_u)
    } else {
        (base.start_u, base.end_u)
    };
    let (start_v, end_v) = if v {
        (base.end_v, base.start_v)
    } else {
        (base.start_v, base.end_v)
    };
    SpriteUv {
        start_u,
        end_u,
        start_v,
        end_v,
    }
}

#[track_caller]
fn same_uv(a: SpriteUv, b: SpriteUv) {
    for (x, y) in [
        (a.start_u, b.start_u),
        (a.end_u, b.end_u),
        (a.start_v, b.start_v),
        (a.end_v, b.end_v),
    ] {
        assert!(
            x.to_bits() == y.to_bits() || (x.is_nan() && y.is_nan()),
            "{a:?} != {b:?}"
        );
    }
}

/// Sizes and positions as `u32` bit patterns, with the small and edge ones
/// as likely as the rest.
fn any_u32() -> impl Strategy<Value = u32> {
    prop_oneof![
        any::<u32>(),
        0u32..4096,
        prop::sample::select(vec![0, 1, 2, 16_777_216, 16_777_217, 0x8000_0000, u32::MAX]),
    ]
}

/// Floats drawn from bit patterns, with the special values as likely.
fn any_f32() -> impl Strategy<Value = f32> {
    prop_oneof![
        any::<u32>().prop_map(f32::from_bits),
        prop::sample::select(vec![
            0.0,
            -0.0,
            1.0,
            -1.0,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NAN,
            f32::from_bits(1),
        ]),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4000))]

    #[test]
    fn edge_flip_swaps_its_own_axis(flip in any::<u32>(), w in any_u32(), h in any_u32(),
                                    x in any_u32(), y in any_u32(), cx in any_u32(), cy in any_u32()) {
        let u = flip & FLIP_U != 0;
        let v = flip & FLIP_V != 0;
        same_uv(sprite_uv(flip), swapped(sprite_uv(0), u, v));
        same_uv(sprite_uv(flip), sprite_uv(flip & (FLIP_U | FLIP_V)));
        same_uv(rect_uv(w, h, flip), swapped(rect_uv(w, h, 0), u, v));
        same_uv(rect_uv(w, h, flip), rect_uv(w, h, flip & (FLIP_U | FLIP_V)));
        same_uv(subsprite_uv(x, y, cx, cy, w, h, flip), swapped(subsprite_uv(x, y, cx, cy, w, h, 0), u, v));
        same_uv(
            subsprite_uv(x, y, cx, cy, w, h, flip),
            subsprite_uv(x, y, cx, cy, w, h, flip & (FLIP_U | FLIP_V)),
        );
    }

    #[test]
    fn edge_subregion_is_one_division_per_edge(x in any_u32(), y in any_u32(), cx in any_u32(),
                                               cy in any_u32(), w in any_u32(), h in any_u32()) {
        #[allow(clippy::cast_precision_loss)]
        let f = |n: u32| n as f32;
        let expected = SpriteUv {
            start_u: f(x) / f(w),
            end_u: (f(x) + f(cx)) / f(w),
            start_v: f(y) / f(h),
            end_v: (f(y) + f(cy)) / f(h),
        };
        same_uv(subsprite_uv(x, y, cx, cy, w, h, 0), expected);
    }

    #[test]
    fn edge_whole_subregion_is_the_plain_sprite(w in 1u32..=u32::MAX, h in 1u32..=u32::MAX, flip in any::<u32>()) {
        prop_assert!(subregion_is_whole(0, 0, w, h, w, h));
        same_uv(subsprite_uv(0, 0, w, h, w, h, flip), sprite_uv(flip));
    }

    #[test]
    fn edge_rect_is_the_size(w in any_u32(), h in any_u32()) {
        #[allow(clippy::cast_precision_loss)]
        let (fw, fh) = (w as f32, h as f32);
        same_uv(rect_uv(w, h, 0), SpriteUv { start_u: 0.0, end_u: fw, start_v: 0.0, end_v: fh });
    }

    #[test]
    fn edge_points_ignore_the_coordinates(cx in any_f32(), cy in any_f32(), a in any_f32(), b in any_f32(),
                                          c in any_f32(), d in any_f32()) {
        let (points, uvs) = sprite(cx, cy, uv(a, b, c, d));
        let (plain, _) = sprite(cx, cy, uv(0.0, 0.0, 0.0, 0.0));
        let bits = |p: &Vec3| [p.x.to_bits(), p.y.to_bits(), p.z.to_bits(), p.w.to_bits()];
        for (got, want) in points.iter().zip(&plain) {
            prop_assert_eq!(bits(got), bits(want));
        }
        prop_assert_eq!(bits(&points[3]), [cx.to_bits(), cy.to_bits(), 0, 0]);
        let uv_bits: Vec<[u32; 2]> = uvs.iter().map(|v| [v.x.to_bits(), v.y.to_bits()]).collect();
        prop_assert_eq!(
            uv_bits,
            vec![
                [a.to_bits(), c.to_bits()],
                [b.to_bits(), c.to_bits()],
                [a.to_bits(), d.to_bits()],
                [b.to_bits(), d.to_bits()],
            ]
        );
    }

    #[test]
    fn edge_quad_rules(has_texture in any::<bool>(), texture_2d in any::<bool>(), w in any_f32(), h in any_f32(),
                       flip in any::<u32>()) {
        let refusal = quad_refusal(has_texture, texture_2d, w, h);
        let want = match (has_texture, texture_2d) {
            (true, true) => QuadRefusal::Draw,
            (true, false) => QuadRefusal::NotTwoD,
            (false, _) if w == 0.0 || h == 0.0 => QuadRefusal::NoSize,
            (false, _) => QuadRefusal::Draw,
        };
        prop_assert_eq!(refusal, want);
        prop_assert_eq!(quad_texture_size(w), w.to_bits() & 0x7fff_ffff == 0);
        prop_assert_eq!(quad_flipped_buffer(flip), flip != 0);
    }

    #[test]
    fn edge_upload_plan(linesize in any_u32(), linesize_out in any_u32(), height in any::<usize>(),
                        flip in any::<bool>()) {
        let plan = upload_plan(linesize, linesize_out, height, flip);
        let row_copy = linesize.min(linesize_out) as usize;
        prop_assert_eq!(plan.row_copy, row_copy);
        let modulus = u128::from(u64::try_from(usize::MAX).unwrap_or(u64::MAX)) + 1;
        let wrap = |v: u128| usize::try_from(v % modulus).unwrap_or(usize::MAX);
        let want = if flip && height == 0 {
            UploadPlan { row_copy, whole_size: 0, src_offset: 0, mode: UploadMode::None }
        } else if flip {
            let offset = wrap((height as u128 - 1) * u128::from(linesize));
            UploadPlan { row_copy, whole_size: 0, src_offset: offset, mode: UploadMode::Flipped }
        } else if linesize == linesize_out {
            let size = wrap(row_copy as u128 * height as u128);
            UploadPlan { row_copy, whole_size: size, src_offset: 0, mode: UploadMode::Whole }
        } else {
            UploadPlan { row_copy, whole_size: 0, src_offset: 0, mode: UploadMode::Rows }
        };
        prop_assert_eq!(plan, want);
    }

    #[test]
    fn edge_pow2(size in any::<u32>()) {
        prop_assert_eq!(is_pow2(size), size >= 2 && size.count_ones() == 1);
    }

    #[test]
    fn edge_mip_plan(width in any_u32(), height in any_u32(), flags in any::<u32>(), levels in any_u32()) {
        let plan = texture_flags(width, height, flags, levels);
        // every flag but GS_BUILD_MIPMAPS is kept
        prop_assert_eq!(plan.flags | BUILD_MIPMAPS, flags | BUILD_MIPMAPS);
        // a warning exactly when mipmaps were asked for and refused
        let asked = flags & BUILD_MIPMAPS != 0 || levels != 1;
        let pow2 = is_pow2(width) && is_pow2(height);
        prop_assert_eq!(plan.warning != MipWarning::Ok, asked && (!pow2 || flags & RENDER_TARGET != 0));
        if plan.warning == MipWarning::Ok {
            prop_assert_eq!((plan.flags, plan.levels), (flags, levels));
        } else {
            prop_assert_eq!((plan.flags & BUILD_MIPMAPS, plan.levels), (0, 1));
        }
        // the plan's own output needs no change
        let again = texture_flags(width, height, plan.flags, plan.levels);
        prop_assert_eq!(again, MipPlan { warning: MipWarning::Ok, ..plan });
    }
}

#[test]
fn edge_pow2_every_shift() {
    for shift in 0..32 {
        let size = 1u32 << shift;
        assert_eq!(is_pow2(size), shift > 0, "{size}");
        for offset in [1u32, 2, 3] {
            let near = size.wrapping_add(offset);
            assert_eq!(is_pow2(near), near >= 2 && near.count_ones() == 1, "{near}");
            let below = size.wrapping_sub(offset);
            assert_eq!(
                is_pow2(below),
                below >= 2 && below.count_ones() == 1,
                "{below}"
            );
        }
    }
}

#[test]
fn edge_mip_plan_every_flag_set() {
    let sizes = [
        (256u32, 256u32),
        (1920, 1080),
        (1, 1),
        (2, 2),
        (0, 2),
        (2, 0),
    ];
    for (width, height) in sizes {
        for levels in [0u32, 1, 2, 5, 13, u32::MAX] {
            for flags in 0u32..128 {
                let plan = texture_flags(width, height, flags, levels);
                let asked = flags & BUILD_MIPMAPS != 0 || levels != 1;
                let pow2 = is_pow2(width) && is_pow2(height);
                let warning = if asked && !pow2 {
                    MipWarning::NotPow2
                } else if asked && flags & RENDER_TARGET != 0 {
                    MipWarning::RenderTarget
                } else {
                    MipWarning::Ok
                };
                assert_eq!(
                    plan.warning, warning,
                    "{width}x{height} {flags:#x} {levels}"
                );
            }
        }
    }
}
