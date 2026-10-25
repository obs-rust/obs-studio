//! `oracle_*` build of `libobs/obs-scene-item-transform.c.h`
//! (oracle/scene_item_transform.c).
//!
//! The cropped size comes back by value as [`OracleItemTransformSize`], the
//! layout of `struct obs_scene_item_transform_size`; the crops cross as
//! `struct obs_sceneitem_crop` ([`OracleCrop`]).

use std::ffi::c_int;

use super::graphics_math::OracleMatrix4;
use super::scene_bounds_place::OracleCrop;
use super::vec2::OracleVec2;

/// `struct obs_scene_item_transform_size`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OracleItemTransformSize {
    pub cx: u32,
    pub cy: u32,
}

/// Index of each entry in `oracle_scene_item_transform_layout`, in the
/// C's order.
pub const LAYOUT_NAMES: [&str; 6] = [
    "sizeof(struct obs_scene_item_transform_size)",
    "alignof(struct obs_scene_item_transform_size)",
    "offsetof(struct obs_scene_item_transform_size, cx)",
    "offsetof(struct obs_scene_item_transform_size, cy)",
    "OBS_BOUNDS_NONE",
    "sizeof(enum obs_bounds_type)",
];

unsafe extern "C" {
    pub fn oracle_obs_scene_item_transform_prepare(
        crop: *mut OracleCrop,
        bounds_crop: *mut OracleCrop,
        last_width: *mut u32,
        last_height: *mut u32,
        width: u32,
        height: u32,
        nested: bool,
    ) -> OracleItemTransformSize;
    pub fn oracle_obs_scene_item_transform_bounded(bounds_type: c_int) -> bool;
    pub fn oracle_obs_scene_item_transform_matrix(
        dst: *mut OracleMatrix4,
        scale: *const OracleVec2,
        origin: *const OracleVec2,
        rot: f32,
        position: *const OracleVec2,
    );
    pub fn oracle_obs_scene_item_transform_box_scale(
        scale: *mut OracleVec2,
        width: u32,
        height: u32,
    );
    /// The entry named `LAYOUT_NAMES[which]`; 0 out of range.
    pub fn oracle_scene_item_transform_layout(which: usize) -> usize;
}
