//! `oracle_*` build of `libobs/obs-video-plan.c.h` (oracle/video_plan.c).
//!
//! [`OracleVideoOutputPlan`] mirrors `struct obs_video_output_plan`, whose
//! layout `oracle_video_plan_layout` reports. The enums cross as `c_int`s.

use core::ffi::{c_char, c_int};

/// `struct obs_video_plane`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OracleVideoPlane {
    pub width: u32,
    pub height: u32,
    pub format: c_int,
}

/// `struct obs_video_output_plan`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct OracleVideoOutputPlan {
    pub conversion_needed: bool,
    pub conversion_techs: [*const c_char; 3],
    pub conversion_width_i: f32,
    pub conversion_height_i: f32,
    pub planes: [OracleVideoPlane; 3],
    pub texture_format: c_int,
    pub render_space: c_int,
}

/// The entries of `oracle_video_plan_layout`, in order.
pub const LAYOUT: [&str; 20] = [
    "sizeof(struct obs_video_plane)",
    "_Alignof(struct obs_video_plane)",
    "offsetof(obs_video_plane, width)",
    "offsetof(obs_video_plane, height)",
    "offsetof(obs_video_plane, format)",
    "sizeof(struct obs_video_output_plan)",
    "_Alignof(struct obs_video_output_plan)",
    "offsetof(obs_video_output_plan, conversion_needed)",
    "offsetof(obs_video_output_plan, conversion_techs)",
    "offsetof(obs_video_output_plan, conversion_width_i)",
    "offsetof(obs_video_output_plan, conversion_height_i)",
    "offsetof(obs_video_output_plan, planes)",
    "offsetof(obs_video_output_plan, texture_format)",
    "offsetof(obs_video_output_plan, render_space)",
    "NUM_CHANNELS",
    "sizeof(enum gs_color_format)",
    "sizeof(enum gs_color_space)",
    "sizeof(enum video_format)",
    "sizeof(enum video_colorspace)",
    "sizeof(enum obs_scale_type)",
];

/// The counts of each enum in `oracle_video_plan_value`, in order:
/// `gs_color_format`, `gs_color_space`, `obs_scale_type`, `video_format`,
/// `video_colorspace`.
pub const VALUE_COUNTS: [usize; 5] = [23, 4, 6, 26, 6];

unsafe extern "C" {
    pub fn oracle_obs_video_size_valid(width: u32, height: u32) -> bool;
    pub fn oracle_obs_video_align_output_size(width: *mut u32, height: *mut u32);
    pub fn oracle_obs_video_output_size_valid(width: u32, height: u32) -> bool;
    pub fn oracle_obs_video_scale_type_name(scale_type: c_int) -> *const c_char;
    pub fn oracle_obs_video_output_plan(
        format: c_int,
        colorspace: c_int,
        width: u32,
        height: u32,
    ) -> OracleVideoOutputPlan;
    /// The entry `which` of [`LAYOUT`]; `usize::MAX` past the end.
    pub fn oracle_video_plan_layout(which: usize) -> usize;
    /// The enum values, grouped as [`VALUE_COUNTS`]; `i64::MIN` past the end.
    pub fn oracle_video_plan_value(which: usize) -> i64;
}
