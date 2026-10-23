//! Safe core of `libobs/obs-scene-extent.c.h`: the content extent of a
//! scene or group.
//!
//! The scene still walks its items, still shifts the children so the
//! content starts at the origin (`update_item_transform`), and still decides
//! what a group does with the result. This is only the fold, driven from C
//! one item at a time, and the size. It keeps no state of its own and calls
//! nothing in libobs.
//!
//! - [`init`] starts at (`M_INFINITE`, `M_INFINITE`) and (-`M_INFINITE`,
//!   -`M_INFINITE`). `M_INFINITE` is 3.4e38, below `f32::MAX`, not infinity.
//! - [`add`] maps the four corners (0, 0), (1, 0), (0, 1) and (1, 1) of one
//!   item's box transform through the matrix, as `vec3_transform` does
//!   ([`crate::group_transform::transform_val`]: the point with z = 0 and
//!   w = 1, dotted with each column in the C's sum order), and folds them
//!   in with the C's strict `<` and `>`. Not `f32::min`/`f32::max`: a NaN
//!   corner fails both compares and changes nothing, and a tie (-0 against
//!   +0) keeps the value already there.
//! - [`size`] is `max - min`, and each side rounded up into a `u32` by
//!   [`size_from_extent`] (`obs_scene_size_from_extent`, saturated in C by
//!   #1840).
//!
//! Characterized, not endorsed (`test_group_extent`), and kept: a NaN
//! anywhere in an axis's column makes every corner NaN on that axis, so the
//! item is skipped there; since the start values are finite, an item at
//! +inf or -inf alone is `u32::MAX` wide, an item wholly past 3.4e38 leaves
//! the start value in place, and the empty fold is -inf wide (0).
//!
//! No intentional differences.

use obs_graphics::matrix4::Matrix4;
use obs_graphics::vec2::Vec2;

use crate::group_transform::transform_val;

/// `M_INFINITE` (`graphics/math-defs.h`).
const M_INFINITE: f32 = 3.4e38;

/// The corners of the unit box, in the C's order.
const CORNERS: [(f32, f32); 4] = [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)];

/// The first float that does not fit in a `uint32_t`, 2^32.
const U32_END: f32 = 4_294_967_296.0;

/// The fold's state, `minv` and `maxv` of `resize_scene_base`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Extent {
    pub min: Vec2,
    pub max: Vec2,
}

/// What `obs_scene_extent_size` computes: `scale` is `max - min`, and `cx`
/// and `cy` are that rounded up into a `uint32_t`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Size {
    pub scale: Vec2,
    pub cx: u32,
    pub cy: u32,
}

/// `obs_scene_extent_init`: min at `M_INFINITE`, max at -`M_INFINITE`.
#[must_use]
pub fn init() -> Extent {
    Extent {
        min: Vec2::new(M_INFINITE, M_INFINITE),
        max: Vec2::new(-M_INFINITE, -M_INFINITE),
    }
}

/// `obs_scene_extent_add`: folds the four corners of `box_transform` into
/// `extent`, in order, with the C's compares.
pub fn add(extent: &mut Extent, box_transform: &Matrix4) {
    for (x, y) in CORNERS {
        let v = transform_val(Vec2::new(x, y), box_transform);
        if v.x < extent.min.x {
            extent.min.x = v.x;
        }
        if v.y < extent.min.y {
            extent.min.y = v.y;
        }
        if v.x > extent.max.x {
            extent.max.x = v.x;
        }
        if v.y > extent.max.y {
            extent.max.y = v.y;
        }
    }
}

/// `obs_scene_extent_size`: `vec2_sub(scale, max, min)`, then each side
/// through [`size_from_extent`].
#[must_use]
pub fn size(extent: &Extent) -> Size {
    let scale = Vec2::new(extent.max.x - extent.min.x, extent.max.y - extent.min.y);
    Size {
        scale,
        cx: size_from_extent(scale.x),
        cy: size_from_extent(scale.y),
    }
}

/// `obs_scene_size_from_extent` (`obs-scene-extent.h`): the extent rounded up,
/// 0 for none, a negative one or NaN, `u32::MAX` from 2^32 on.
#[must_use]
pub fn size_from_extent(extent: f32) -> u32 {
    let size = extent.ceil();
    if size.is_nan() || size <= 0.0 {
        return 0;
    }
    if size >= U32_END {
        return u32::MAX;
    }
    // In (0, 2^32) and whole, so it fits exactly.
    size as u32
}
