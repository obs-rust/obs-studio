//! `oracle_*` build of `libobs/obs-video-copy-plan.c.h`
//! (oracle/video_copy_plan.c): the output frame copy plan and the scale
//! effect choice, renamed.
//!
//! The plan and the copy come back as [`OracleCopyPlan`] and [`OracleCopy`];
//! the scale input and the reuse keys are untyped pointers that callers lay
//! out themselves. [`oracle_video_copy_plan_layout`] says how the C sees all
//! four structs. The enums cross as `c_int`s.

use core::ffi::{c_int, c_void};

/// Index of each entry in `oracle_video_copy_plan_constant`, in the C's
/// order.
pub const CONSTANT_NAMES: [&str; 16] = [
    "OBS_VIDEO_COPY_MAX",
    "OBS_VIDEO_COPY_SCALE_DISABLE",
    "OBS_VIDEO_COPY_SCALE_POINT",
    "OBS_VIDEO_COPY_SCALE_BICUBIC",
    "OBS_VIDEO_COPY_SCALE_BILINEAR",
    "OBS_VIDEO_COPY_SCALE_LANCZOS",
    "OBS_VIDEO_COPY_SCALE_AREA",
    "OBS_VIDEO_SCALE_EFFECT_DEFAULT",
    "OBS_VIDEO_SCALE_EFFECT_BILINEAR_LOWRES",
    "OBS_VIDEO_SCALE_EFFECT_BICUBIC",
    "OBS_VIDEO_SCALE_EFFECT_LANCZOS",
    "OBS_VIDEO_SCALE_EFFECT_AREA",
    "OBS_VIDEO_SCALE_LOADED(OBS_VIDEO_SCALE_EFFECT_DEFAULT)",
    "OBS_VIDEO_SCALE_LOADED(OBS_VIDEO_SCALE_EFFECT_AREA)",
    "MAX_AV_PLANES",
    "VIDEO_FORMAT_R10L",
];

/// Index of each entry in `oracle_video_copy_plan_layout`, in the C's
/// order.
pub const LAYOUT_NAMES: [&str; 31] = [
    "sizeof(struct obs_video_copy)",
    "alignof(struct obs_video_copy)",
    "offsetof(struct obs_video_copy, src_after_plane0)",
    "offsetof(struct obs_video_copy, whole)",
    "offsetof(struct obs_video_copy, bytes)",
    "offsetof(struct obs_video_copy, rows)",
    "offsetof(struct obs_video_copy, src_stride)",
    "offsetof(struct obs_video_copy, dst_stride)",
    "sizeof(struct obs_video_copy_plan)",
    "alignof(struct obs_video_copy_plan)",
    "offsetof(struct obs_video_copy_plan, count)",
    "offsetof(struct obs_video_copy_plan, copies)",
    "sizeof(struct obs_video_scale_input)",
    "alignof(struct obs_video_scale_input)",
    "offsetof(struct obs_video_scale_input, base_width)",
    "offsetof(struct obs_video_scale_input, base_height)",
    "offsetof(struct obs_video_scale_input, target_width)",
    "offsetof(struct obs_video_scale_input, target_height)",
    "offsetof(struct obs_video_scale_input, output_width)",
    "offsetof(struct obs_video_scale_input, output_height)",
    "offsetof(struct obs_video_scale_input, scale_type)",
    "offsetof(struct obs_video_scale_input, loaded)",
    "sizeof(struct obs_video_mix_reuse_key)",
    "alignof(struct obs_video_mix_reuse_key)",
    "offsetof(struct obs_video_mix_reuse_key, view)",
    "offsetof(struct obs_video_mix_reuse_key, render_space)",
    "offsetof(struct obs_video_mix_reuse_key, base_width)",
    "offsetof(struct obs_video_mix_reuse_key, base_height)",
    "offsetof(struct obs_video_mix_reuse_key, rendered)",
    "sizeof(enum obs_video_scale_effect)",
    "sizeof(enum video_format)",
];

/// `struct obs_video_copy`, as the C returns it.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OracleCopy {
    pub src_after_plane0: bool,
    pub whole: bool,
    pub bytes: usize,
    pub rows: u32,
    pub src_stride: u32,
    pub dst_stride: u32,
}

/// `struct obs_video_copy_plan`, as the C returns it.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OracleCopyPlan {
    pub count: usize,
    pub copies: [OracleCopy; 3],
}

unsafe extern "C" {
    /// `format` is an `enum video_format` value; the line sizes point to at
    /// least three `uint32_t`s each.
    pub fn oracle_obs_video_gpu_copy_plan(
        format: c_int,
        width: u32,
        height: u32,
        in_linesize: *const u32,
        out_linesize: *const u32,
    ) -> OracleCopyPlan;

    pub fn oracle_obs_video_rgbx_copy_plan(
        width: u32,
        height: u32,
        in_linesize: u32,
        out_linesize: u32,
    ) -> OracleCopy;

    pub fn oracle_obs_video_scale_resolution_close(
        base_width: u32,
        base_height: u32,
        width: u32,
        height: u32,
    ) -> bool;

    /// `input` points to a `struct obs_video_scale_input`; returns an `enum
    /// obs_video_scale_effect` value.
    pub fn oracle_obs_video_scale_effect_choice(input: *const c_void) -> u32;

    /// `other` and `mix` point to `struct obs_video_mix_reuse_key`s.
    pub fn oracle_obs_video_mix_reuse_match(other: *const c_void, mix: *const c_void) -> bool;

    /// The entry named `CONSTANT_NAMES[which]`; -1 out of range.
    pub fn oracle_video_copy_plan_constant(which: usize) -> i64;

    /// The entry named `LAYOUT_NAMES[which]`; `usize::MAX` out of range.
    pub fn oracle_video_copy_plan_layout(which: usize) -> usize;
}
