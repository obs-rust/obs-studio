//! Safe core of `libobs/obs-scene-extent.c.h`: the content extent of a
//! scene or group.
//!
//! The scene still walks its items, still shifts the children so the
//! content starts at the origin (`update_item_transform`), and still decides
//! what a group does with the result. This is only the fold, driven from C
//! one item at a time, and the size.

use obs_graphics::matrix4::Matrix4;
use obs_graphics::vec2::Vec2;

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

/// `obs_scene_extent_init`.
#[must_use]
pub fn init() -> Extent {
    todo!()
}

/// `obs_scene_extent_add`.
pub fn add(extent: &mut Extent, box_transform: &Matrix4) {
    let _ = (extent, box_transform);
    todo!()
}

/// `obs_scene_extent_size`.
#[must_use]
pub fn size(extent: &Extent) -> Size {
    let _ = extent;
    todo!()
}

/// `obs_scene_size_from_extent` (`obs-scene.h`).
#[must_use]
pub fn size_from_extent(extent: f32) -> u32 {
    let _ = extent;
    todo!()
}
