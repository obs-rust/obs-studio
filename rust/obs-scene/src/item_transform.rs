//! Safe core of `libobs/obs-scene-item-transform.c.h`: the scene item
//! transform composition of `compute_item_transform()` in `obs-scene.c.h`,
//! as the steps the scene calls in order:
//! - [`prepare`]: the bounds crop reset, a nested scene's crop rescaled by
//!   its size change (`update_nested_scene_crop`), the cropped size
//!   (`calc_cx` and `calc_cy`) and the last size ([`Prepared`]);
//! - [`bounded`]: whether the item has a bounding box, so its size and box
//!   scale come from the bounds and not the scale;
//! - [`matrix`]: `draw_transform` and `box_transform`, the same
//!   `obs-graphics` matrix calls in the same order as the C's `matrix4_*`
//!   (scale, minus the origin, the rotation about z, the position);
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
//! C's `float` constant and one multiply; the matrices are the
//! `obs-graphics` `Matrix4` operations the hybrid libobs's `matrix4_*`
//! shims run, in the C's order. There is no `f64` intermediate and Rust
//! never fuses a multiply and an add.
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

use crate::bounds_place::Crop;

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
    let _ = (crop, last_width, last_height, width, height, nested);
    todo!("obs_scene_item_transform_prepare")
}

/// `obs_scene_item_transform_bounded`: `bounds_type` (an `enum
/// obs_bounds_type` as the C's `int`) is not `OBS_BOUNDS_NONE`.
#[must_use]
pub fn bounded(bounds_type: i32) -> bool {
    let _ = bounds_type;
    todo!("obs_scene_item_transform_bounded")
}

/// `obs_scene_item_transform_matrix`: the identity, scaled by
/// `(scale.x, scale.y, 1)`, translated by `(-origin.x, -origin.y, 0)`,
/// rotated by `rot * RAD` about `(0, 0, 1)` and translated by
/// `(position.x, position.y, 0)`.
#[must_use]
pub fn matrix(scale: Vec2, origin: Vec2, rot: f32, position: Vec2) -> Matrix4 {
    let _ = (scale, origin, rot, position);
    todo!("obs_scene_item_transform_matrix")
}

/// `obs_scene_item_transform_box_scale`: `scale.x * (float)width` and
/// `scale.y * (float)height`, in `f32`.
#[must_use]
pub fn box_scale(scale: Vec2, width: u32, height: u32) -> Vec2 {
    let _ = (scale, width, height);
    todo!("obs_scene_item_transform_box_scale")
}
