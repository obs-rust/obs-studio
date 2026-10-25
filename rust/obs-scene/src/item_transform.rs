//! Safe core of `libobs/obs-scene-item-transform.c.h`: the scene item
//! transform composition of `compute_item_transform()` in `obs-scene.c.h`,
//! as the steps the scene calls in order:
//! - [`prepare`]: the bounds crop reset, a nested scene's crop rescaled by
//!   its size change (`update_nested_scene_crop`), the cropped size
//!   (`calc_cx` and `calc_cy`) and the last size ([`Prepared`]);
//! - [`bounded`]: whether the item has a bounding box, so its size and box
//!   scale come from the bounds and not the scale;
//! - [`matrix`]: `draw_transform` and `box_transform`, the C's
//!   `matrix4_*` calls in the same order (scale, minus the origin, the
//!   rotation about z, the position);
//! - [`box_scale`]: the box scale of an item without bounds.
//!
//! `obs-scene.c.h` keeps every `get_scene_dimensions()` read between these
//! steps (`item_canvas_scale()`, `pos_to_absolute()` and
//! `size_to_absolute()`, each where it was, none hoisted),
//! `calculate_bounds_data()` (a read of the bounds, then the ported
//! `obs_bounds_place()`), `add_alignment()` and all six float-to-integer
//! casts: the two `(uint32_t)((float)cx * scale.x)` of an item without
//! bounds and the four `(int)` passed to `add_alignment()`. A negative
//! float cast to `uint32_t` (a flipped item) or a float past `INT_MAX`
//! cast to `int` is undefined in C: x86-64 wraps, arm64 saturates, and
//! Rust's `as` saturates. So no float becomes an integer here, and each
//! architecture keeps the C's result. `update_item_transform()` keeps the
//! defer check, the source size read, the parent's video lock and the
//! `item_transform` signal.
//!
//! Floats are bit-equal to the C: the crop factors are one `f32` division
//! of the sizes converted as C's `(float)uint32_t` is (nearest, ties to
//! even); the box scale is one `f32` multiply per axis; [`RAD`] is the
//! C's `float` constant and one multiply. The matrices are the C's
//! `matrix4_*` operations in the C's order: `matrix4_mul` and `vec4_dot`
//! are `group_transform`'s (the SSE sum order, as `obs-graphics`' and the
//! hybrid libobs's), and `quat_from_axisang()` and `matrix4_from_quat()`
//! are written out here, with `sinf` and `cosf` called as the host's C
//! calls them. They are not `obs-graphics` calls, so the `obs-scene`
//! object does not pull `obs-graphics`' render code (and its `gs_*`
//! references) into the tests that link `libobs_rust-static` alone, as
//! `group_transform` and `extent` keep it. There is no `f64`
//! intermediate and Rust never fuses a multiply and an add.
//!
//! Characterized, not endorsed (`test_scene_item_transform_steps`), and
//! kept: any bounds type but 0 is bounded (a value outside the enum is
//! `OBS_BOUNDS_NONE` before this runs, #2281); a negative crop scales
//! negative; a crop scaled past the `i32` range saturates; a crop of more
//! than the whole source leaves 2.
//!
//! No intentional differences. Nothing here keeps state, takes a lock or
//! calls libobs.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::unreachable
)]

use obs_graphics::matrix4::Matrix4;
use obs_graphics::vec2::Vec2;
use obs_graphics::vec4::Vec4;

use crate::bounds_place::Crop;
use crate::crop::{cropped_size, scale_crop};
use crate::group_transform::{dot, matrix_mul};
use crate::platform;

/// `RAD()` of `graphics/math-defs.h`: degrees to radians, one `f32`
/// multiply by this `float` constant.
pub const RAD: f32 = 0.017_453_292_519_943_295_769_236_907_684_8;

/// What [`prepare`] leaves: the item's crop (rescaled for a nested scene),
/// its bounds crop (zeroed), the last size (the source size) and the size
/// left after the crops.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Prepared {
    pub crop: Crop,
    pub bounds_crop: Crop,
    pub last_width: u32,
    pub last_height: u32,
    pub cx: u32,
    pub cy: u32,
}

/// `obs_scene_item_transform_prepare`. `crop`: the item's crop.
/// `last_width` and `last_height`: the size it was last composed at.
/// `width` and `height`: the source's size. `nested`: `update_tex` and
/// the item is a scene.
///
/// When nested and neither last size is 0, each crop is scaled by the new
/// size over the last one (`(float)width / (float)last_width`, then
/// [`crate::crop::scale_crop`]). The size left is
/// [`crate::crop::cropped_size`] with the bounds crop just zeroed.
#[must_use]
pub fn prepare(
    crop: Crop,
    last_width: u32,
    last_height: u32,
    width: u32,
    height: u32,
    nested: bool,
) -> Prepared {
    // Reset bounds crop
    let bounds_crop = Crop {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };

    // Adjust crop on nested scenes (if any)
    let crop = if nested {
        nested_crop(crop, last_width, last_height, width, height)
    } else {
        crop
    };

    Prepared {
        crop,
        bounds_crop,
        last_width: width,
        last_height: height,
        cx: cropped_size(
            width,
            crop.left,
            crop.right,
            bounds_crop.left,
            bounds_crop.right,
        ),
        cy: cropped_size(
            height,
            crop.top,
            crop.bottom,
            bounds_crop.top,
            bounds_crop.bottom,
        ),
    }
}

/// `update_nested_scene_crop`: the crop scaled by the new size over the
/// last one, unless a last size is 0 (the scene was never sized).
fn nested_crop(crop: Crop, last_width: u32, last_height: u32, width: u32, height: u32) -> Crop {
    if last_height == 0 || last_width == 0 {
        return crop;
    }

    // Use last size and new size to calculate factor to adjust crop by.
    // `as f32` is C's (float)uint32_t: the nearest float, ties to even.
    #[allow(clippy::cast_precision_loss)]
    let (scale_x, scale_y) = (
        width as f32 / last_width as f32,
        height as f32 / last_height as f32,
    );

    Crop {
        left: scale_crop(crop.left, scale_x),
        top: scale_crop(crop.top, scale_y),
        right: scale_crop(crop.right, scale_x),
        bottom: scale_crop(crop.bottom, scale_y),
    }
}

/// `obs_scene_item_transform_bounded`: `bounds_type` (an `enum
/// obs_bounds_type` as the C's `int`) is not `OBS_BOUNDS_NONE`.
#[must_use]
pub fn bounded(bounds_type: i32) -> bool {
    // OBS_BOUNDS_NONE
    bounds_type != 0
}

/// `obs_scene_item_transform_matrix`: the identity, scaled by
/// `(scale.x, scale.y, 1)`, translated by `(-origin.x, -origin.y, 0)`,
/// rotated by `rot * RAD` about `(0, 0, 1)` and translated by
/// `(position.x, position.y, 0)`.
#[must_use]
pub fn matrix(scale: Vec2, origin: Vec2, rot: f32, position: Vec2) -> Matrix4 {
    // matrix4_identity(), then each matrix4_* in the C's order; each one
    // post-multiplies (`matrix4_mul(dst, m, &temp)`).
    let m = identity();
    // matrix4_scale3f(): vec3_set(x, y, 1), then matrix4_scale()
    let m = matrix_mul(
        &m,
        &Matrix4::new(
            Vec4::new(scale.x, 0.0, 0.0, 0.0),
            Vec4::new(0.0, scale.y, 0.0, 0.0),
            Vec4::new(0.0, 0.0, 1.0, 0.0),
            Vec4::new(0.0, 0.0, 0.0, 1.0),
        ),
    );
    let m = matrix_mul(&m, &translation(-origin.x, -origin.y));
    let m = matrix_mul(&m, &axis_angle(0.0, 0.0, 1.0, rad(rot)));
    matrix_mul(&m, &translation(position.x, position.y))
}

/// `obs_scene_item_transform_box_scale`: `scale.x * (float)width` and
/// `scale.y * (float)height`, in `f32`.
#[must_use]
pub fn box_scale(scale: Vec2, width: u32, height: u32) -> Vec2 {
    #[allow(clippy::cast_precision_loss)]
    let (width, height) = (width as f32, height as f32);
    Vec2::new(scale.x * width, scale.y * height)
}

/// `matrix4_identity()`.
fn identity() -> Matrix4 {
    Matrix4::new(
        Vec4::new(1.0, 0.0, 0.0, 0.0),
        Vec4::new(0.0, 1.0, 0.0, 0.0),
        Vec4::new(0.0, 0.0, 1.0, 0.0),
        Vec4::new(0.0, 0.0, 0.0, 1.0),
    )
}

/// The `temp` of `matrix4_translate3f()` (`matrix4_translate3v()`): the
/// identity with `vec4_from_vec3(x, y, 0)` (w = 1) as `t`.
fn translation(x: f32, y: f32) -> Matrix4 {
    Matrix4::new(
        Vec4::new(1.0, 0.0, 0.0, 0.0),
        Vec4::new(0.0, 1.0, 0.0, 0.0),
        Vec4::new(0.0, 0.0, 1.0, 0.0),
        Vec4::new(x, y, 0.0, 1.0),
    )
}

/// The `temp` of `matrix4_rotate_aa4f()` (`matrix4_rotate_aa()`):
/// `matrix4_from_axisang()`, which is `quat_from_axisang()` then
/// `matrix4_from_quat()`, written out with their operations in their
/// order: `halfa = w * 0.5`, separate `sinf` and `cosf` as the host's C
/// calls them, each axis lane times the sine (so a 0 axis lane is -0 for
/// a negative sine), the norm as `quat_dot`'s SSE sum, `2 / norm` (0 for a
/// norm that is not above 0), and `1 - (yy + zz)` and the rest.
#[allow(clippy::many_single_char_names)]
fn axis_angle(x: f32, y: f32, z: f32, w: f32) -> Matrix4 {
    let halfa = w * 0.5;
    let (sine, cosine) = platform::sin_cos(halfa);
    let q = Vec4::new(x * sine, y * sine, z * sine, cosine);

    let norm = dot(q, q);
    let s = if norm > 0.0 { 2.0 / norm } else { 0.0 };

    let xx = q.x * q.x * s;
    let yy = q.y * q.y * s;
    let zz = q.z * q.z * s;
    let xy = q.x * q.y * s;
    let xz = q.x * q.z * s;
    let yz = q.y * q.z * s;
    let wx = q.w * q.x * s;
    let wy = q.w * q.y * s;
    let wz = q.w * q.z * s;

    Matrix4::new(
        Vec4::new(1.0 - (yy + zz), xy + wz, xz - wy, 0.0),
        Vec4::new(xy - wz, 1.0 - (xx + zz), yz + wx, 0.0),
        Vec4::new(xz + wy, yz - wx, 1.0 - (xx + yy), 0.0),
        Vec4::new(0.0, 0.0, 0.0, 1.0),
    )
}

/// `RAD(rot)`: one `f32` multiply.
fn rad(degrees: f32) -> f32 {
    degrees * RAD
}
