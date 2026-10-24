//! `oracle_*` build of `libobs/obs-source-frame-copy.c.h`
//! (oracle/source_frame_copy.c): the async frame copy plan and cache
//! decisions, renamed.
//!
//! The header's structs are mirrored here, passed and returned by value as
//! the C does; `oracle_source_frame_copy_layout` reports their layout and
//! the constants. The enums cross as `c_int`s, the step as the C's enum
//! value.

use core::ffi::{c_int, c_long};

/// `OBS_SOURCE_FRAME_COPY_PLANES`.
pub const PLANES: usize = 4;

/// `struct obs_source_frame_copy_lines`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OracleLines {
    pub linesize: [u32; PLANES],
}

/// `struct obs_source_frame_plane_copy`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OraclePlaneCopy {
    pub whole: bool,
    pub bytes: usize,
    pub rows: u32,
    pub src_stride: u32,
    pub dst_stride: u32,
}

/// `struct obs_source_frame_copy_plan`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OracleCopyPlan {
    pub color_range: bool,
    pub count: usize,
    pub planes: [OraclePlaneCopy; PLANES],
}

/// `struct obs_source_frame_copy_key`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OracleKey {
    pub width: u32,
    pub height: u32,
    pub convert_type: c_int,
}

/// `struct obs_source_frame_cache_age`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OracleCacheAge {
    pub counted: bool,
    pub unused_count: c_long,
    pub drop: bool,
}

/// `struct obs_source_frame_copy_shape`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OracleShape {
    pub format: c_int,
    pub width: u32,
    pub height: u32,
}

/// The entries of `oracle_source_frame_copy_layout`, in order.
pub const LAYOUT: [&str; 46] = [
    "sizeof(struct obs_source_frame_copy_lines)",
    "_Alignof(struct obs_source_frame_copy_lines)",
    "offsetof(lines, linesize)",
    "sizeof(struct obs_source_frame_plane_copy)",
    "_Alignof(struct obs_source_frame_plane_copy)",
    "offsetof(plane_copy, whole)",
    "offsetof(plane_copy, bytes)",
    "offsetof(plane_copy, rows)",
    "offsetof(plane_copy, src_stride)",
    "offsetof(plane_copy, dst_stride)",
    "sizeof(struct obs_source_frame_copy_plan)",
    "_Alignof(struct obs_source_frame_copy_plan)",
    "offsetof(plan, color_range)",
    "offsetof(plan, count)",
    "offsetof(plan, planes)",
    "sizeof(struct obs_source_frame_copy_key)",
    "_Alignof(struct obs_source_frame_copy_key)",
    "offsetof(key, width)",
    "offsetof(key, height)",
    "offsetof(key, convert_type)",
    "sizeof(struct obs_source_frame_cache_age)",
    "_Alignof(struct obs_source_frame_cache_age)",
    "offsetof(age, counted)",
    "offsetof(age, unused_count)",
    "offsetof(age, drop)",
    "sizeof(struct obs_source_frame_copy_shape)",
    "_Alignof(struct obs_source_frame_copy_shape)",
    "offsetof(shape, format)",
    "offsetof(shape, width)",
    "offsetof(shape, height)",
    "sizeof(enum obs_source_frame_cache_step)",
    "OBS_SOURCE_FRAME_CACHE_KEEP",
    "OBS_SOURCE_FRAME_CACHE_RESET",
    "OBS_SOURCE_FRAME_CACHE_OVERFLOW",
    "OBS_SOURCE_FRAME_COPY_PLANES",
    "OBS_SOURCE_FRAME_COPY_MAX_ASYNC_FRAMES",
    "OBS_SOURCE_FRAME_COPY_MAX_UNUSED",
    "sizeof(long)",
    "sizeof(size_t)",
    "sizeof(int)",
    "sizeof(bool)",
    "sizeof(enum video_format)",
    "sizeof(enum video_range_type)",
    "VIDEO_RANGE_DEFAULT",
    "VIDEO_RANGE_PARTIAL",
    "VIDEO_RANGE_FULL",
];

unsafe extern "C" {
    /// `obs_source_frame_copy_plan`.
    pub fn oracle_obs_source_frame_copy_plan(
        format: c_int,
        full_range: bool,
        height: u32,
        src: OracleLines,
        dst: OracleLines,
    ) -> OracleCopyPlan;
    /// `obs_source_frame_copy_cache_step`: the step as the C's enum value.
    pub fn oracle_obs_source_frame_copy_cache_step(
        queued: usize,
        cache: OracleKey,
        frame: OracleKey,
    ) -> u32;
    /// `obs_source_frame_copy_cache_clean`. Undefined in C for
    /// `unused_count == LONG_MAX` with `used` false (`++` overflows).
    pub fn oracle_obs_source_frame_copy_cache_clean(
        used: bool,
        unused_count: c_long,
    ) -> OracleCacheAge;
    /// `obs_source_frame_copy_full_range`.
    pub fn oracle_obs_source_frame_copy_full_range(format: c_int, full_range: bool) -> bool;
    /// `obs_source_frame_copy_full_range2`.
    pub fn oracle_obs_source_frame_copy_full_range2(format: c_int, range: c_int) -> bool;
    /// `obs_source_frame_copy_preload_changed`.
    pub fn oracle_obs_source_frame_copy_preload_changed(
        has_preload: bool,
        preload: OracleShape,
        frame: OracleShape,
    ) -> bool;
    /// The entry named `LAYOUT[which]`; `usize::MAX` past the end.
    pub fn oracle_source_frame_copy_layout(which: usize) -> usize;
    /// `enum video_format` value `which` in enum order; `i32::MIN` past the
    /// end.
    pub fn oracle_source_frame_copy_format(which: usize) -> c_int;
}
