//! Safe core of `libobs/obs-scene-group-transform.c.h`.
//!
//! The scene still reads the item, still converts absolute and relative
//! coordinates, still inverts the draw transform when an item joins a
//! group, and still calls `update_item_transform`. This is only the matrix
//! math.
//!
//! `vec4_len` is the header inline: the SSE sum order of `vec4_dot`, then
//! `sqrtf` when that dot is strictly positive, otherwise positive zero.
//! The sum is written out here, and so is the scale multiply. A call into
//! `obs-graphics` can land in the codegen unit that contains `ep_free`.
//! `test_scene_bounds_place_rust` links this archive against `libobs.so`,
//! which keeps the effect-parser adapter symbols hidden, so that unit
//! does not link.
//!
//! A scale that is not greater than zero takes the minus sign, so a zero
//! scale becomes negative zero. A rotation in the matrix does not change
//! the scale, because the scale is the column length. The C writes
//! `x_base` and never reads it. Nothing observes that store.

use obs_graphics::matrix4::Matrix4;
use obs_graphics::vec2::Vec2;
use obs_graphics::vec4::Vec4;

/// `obs_scene_transform_val`. A point (`w` = 1) through `transform`.
#[must_use]
pub fn transform_val(point: Vec2, transform: &Matrix4) -> Vec2 {
    // `vec4_from_vec3` sets `w` to 1. `vec4_transform` dots that point
    // with each column. Only x and y are kept.
    let point = Vec4::new(point.x, point.y, 0.0, 1.0);
    Vec2::new(
        dot(point, column(transform, 0)),
        dot(point, column(transform, 1)),
    )
}

/// Position, scale, and rotation after `obs_scene_group_transform`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GroupTransform {
    pub pos: Vec2,
    pub scale: Vec2,
    pub rot: f32,
}

/// `obs_scene_group_transform`.
///
/// Translation moves `pos` only. It is cleared before the scale matrix is
/// multiplied in. Each scale axis keeps its sign. `rot_delta` is added to
/// `rot` and is the only change to the rotation.
#[must_use]
pub fn group_transform(
    transform: &Matrix4,
    pos: Vec2,
    scale: Vec2,
    rot: f32,
    rot_delta: f32,
) -> GroupTransform {
    let pos = transform_val(pos, transform);
    let mut linear = *transform;
    linear.t = Vec4::new(0.0, 0.0, 0.0, 1.0);
    let scaled = matrix_mul(
        &Matrix4::new(
            Vec4::new(scale.x, 0.0, 0.0, 0.0),
            Vec4::new(0.0, scale.y, 0.0, 0.0),
            Vec4::new(0.0, 0.0, 1.0, 0.0),
            Vec4::new(0.0, 0.0, 0.0, 1.0),
        ),
        &linear,
    );
    GroupTransform {
        pos,
        scale: Vec2::new(signed_len(scaled.x, scale.x), signed_len(scaled.y, scale.y)),
        rot: rot + rot_delta,
    }
}

/// `matrix4_mul`. `m1` is the left matrix. Each output lane is `vec4_dot`
/// of an `m1` row with an `m2` column.
pub(crate) fn matrix_mul(m1: &Matrix4, m2: &Matrix4) -> Matrix4 {
    let row = |r: Vec4| {
        Vec4::new(
            dot(r, column(m2, 0)),
            dot(r, column(m2, 1)),
            dot(r, column(m2, 2)),
            dot(r, column(m2, 3)),
        )
    };
    Matrix4::new(row(m1.x), row(m1.y), row(m1.z), row(m1.t))
}

fn column(m: &Matrix4, index: usize) -> Vec4 {
    let lane = |row: Vec4| match index {
        0 => row.x,
        1 => row.y,
        2 => row.z,
        _ => row.w,
    };
    Vec4::new(lane(m.x), lane(m.y), lane(m.z), lane(m.t))
}

/// Header `vec4_dot`: `(w + y) + (z + x)` after the products.
pub(crate) fn dot(a: Vec4, b: Vec4) -> f32 {
    let m0 = a.x * b.x;
    let m1 = a.y * b.y;
    let m2 = a.z * b.z;
    let m3 = a.w * b.w;
    (m3 + m1) + (m2 + m0)
}

/// Header `vec4_len`: positive zero when the dot is not strictly positive.
fn vec4_len(v: Vec4) -> f32 {
    let dot = dot(v, v);
    if dot > 0.0 { dot.sqrt() } else { 0.0 }
}

fn signed_len(column: Vec4, axis: f32) -> f32 {
    let sign = if axis > 0.0 { 1.0 } else { -1.0 };
    vec4_len(column) * sign
}
