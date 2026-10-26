//! `oracle_*` build of `libobs/obs-source-video-tick.c.h`
//! (oracle/source_video_tick.c): the per-frame decisions of
//! `obs_source_video_tick()` and `async_tick()`, renamed. The `bool`
//! results are declared as `u8`, the byte the C returns, so a parity test
//! compares it without assuming it is 0 or 1; the enums as [`c_uint`], the
//! width the C returns them in (their values are 0 to 2, so `int` and
//! `unsigned int` agree); the counts as [`c_long`].

use std::os::raw::{c_int, c_long, c_uint};

/// Index of each entry in `oracle_source_video_tick_value`, in the C's
/// order.
pub const VALUE_NAMES: [&str; 11] = [
    "OBS_SOURCE_VIDEO_TICK_TYPE_TRANSITION",
    "OBS_SOURCE_VIDEO_TICK_ASYNC",
    "OBS_SOURCE_VIDEO_TICK_CONTROLLABLE_MEDIA",
    "OBS_SOURCE_VIDEO_TICK_EDGE_NONE",
    "OBS_SOURCE_VIDEO_TICK_EDGE_ON",
    "OBS_SOURCE_VIDEO_TICK_EDGE_OFF",
    "OBS_SOURCE_VIDEO_TICK_ASYNC_CLOSEST",
    "OBS_SOURCE_VIDEO_TICK_ASYNC_DEINTERLACE",
    "sizeof(enum obs_source_video_tick_edge)",
    "sizeof(enum obs_source_video_tick_async_route)",
    "sizeof(long)",
];

unsafe extern "C" {
    /// `bool obs_source_video_tick_transition(int type)`.
    pub fn oracle_obs_source_video_tick_transition(source_type: c_int) -> u8;
    /// `bool obs_source_video_tick_async(uint32_t output_flags)`.
    pub fn oracle_obs_source_video_tick_async(output_flags: u32) -> u8;
    /// `bool obs_source_video_tick_media(uint32_t output_flags)`.
    pub fn oracle_obs_source_video_tick_media(output_flags: u32) -> u8;
    /// `bool obs_source_video_tick_deferred_update(long
    /// defer_update_count)`.
    pub fn oracle_obs_source_video_tick_deferred_update(defer_update_count: c_long) -> u8;
    /// `enum obs_source_video_tick_edge obs_source_video_tick_edge(long
    /// refs, bool was)`.
    pub fn oracle_obs_source_video_tick_edge(refs: c_long, was: bool) -> c_uint;
    /// `bool obs_source_video_tick_calls_tick(bool has_data, bool
    /// has_callback)`.
    pub fn oracle_obs_source_video_tick_calls_tick(has_data: bool, has_callback: bool) -> u8;
    /// `enum obs_source_video_tick_async_route
    /// obs_source_video_tick_async_route(bool deinterlacing)`.
    pub fn oracle_obs_source_video_tick_async_route(deinterlacing: bool) -> c_uint;
    /// The header's values and sizes, by index into [`VALUE_NAMES`];
    /// `INT64_MIN` past the end.
    pub fn oracle_source_video_tick_value(which: usize) -> i64;
}
