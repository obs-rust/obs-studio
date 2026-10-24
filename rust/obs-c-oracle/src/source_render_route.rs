//! `oracle_*` build of `libobs/obs-source-render-route.c.h`
//! (oracle/source_render_route.c): the source and filter render routing,
//! renamed. The `bool` results are declared as `u8`, the byte the C
//! returns, so a parity test compares it without assuming it is 0 or 1;
//! the enums as `u32`, their value.

use std::os::raw::{c_char, c_int};

/// Index of each entry in `oracle_source_render_route_value`, in the C's
/// order.
pub const VALUE_NAMES: [&str; 44] = [
    "OBS_SOURCE_TYPE_INPUT",
    "OBS_SOURCE_TYPE_FILTER",
    "OBS_SOURCE_TYPE_TRANSITION",
    "OBS_SOURCE_TYPE_SCENE",
    "OBS_SOURCE_VIDEO",
    "OBS_SOURCE_AUDIO",
    "OBS_SOURCE_ASYNC",
    "OBS_SOURCE_CUSTOM_DRAW",
    "OBS_SOURCE_CAP_DISABLED",
    "OBS_SOURCE_SRGB",
    "OBS_NO_DIRECT_RENDERING",
    "OBS_ALLOW_DIRECT_RENDERING",
    "GS_CS_SRGB",
    "GS_CS_SRGB_16F",
    "GS_CS_709_EXTENDED",
    "GS_CS_709_SCRGB",
    "GS_RG16",
    "OBS_SOURCE_RENDER_ROUTE_FILTER_READY",
    "OBS_SOURCE_RENDER_ROUTE_FILTER_NO_TARGET",
    "OBS_SOURCE_RENDER_ROUTE_FILTER_NO_PARENT",
    "OBS_SOURCE_RENDER_ROUTE_DRAW_DEFAULT",
    "OBS_SOURCE_RENDER_ROUTE_DRAW_FILTERS",
    "OBS_SOURCE_RENDER_ROUTE_DRAW_MAIN",
    "OBS_SOURCE_RENDER_ROUTE_DRAW_TARGET",
    "OBS_SOURCE_RENDER_ROUTE_DRAW_DEINTERLACE",
    "OBS_SOURCE_RENDER_ROUTE_DRAW_ASYNC",
    "OBS_SOURCE_RENDER_ROUTE_DRAW_READ_RENDERING_FILTER",
    "OBS_SOURCE_RENDER_ROUTE_DRAW_READ_VIDEO_RENDER",
    "OBS_SOURCE_RENDER_ROUTE_DRAW_READ_FILTER_TARGET",
    "OBS_SOURCE_RENDER_ROUTE_DRAW_READ_DEINTERLACING",
    "OBS_SOURCE_RENDER_ROUTE_DRAW_READ_TARGET_VIDEO_RENDER",
    "OBS_SOURCE_RENDER_ROUTE_DRAW_READ_TARGET_DEINTERLACING",
    "OBS_SOURCE_RENDER_ROUTE_TECH_END_BYPASS",
    "OBS_SOURCE_RENDER_ROUTE_TECH_END_TEXTURE",
    "sizeof(enum obs_source_type)",
    "sizeof(enum obs_allow_direct_render)",
    "sizeof(enum gs_color_space)",
    "sizeof(enum gs_color_format)",
    "sizeof(enum obs_source_render_route_filter)",
    "sizeof(enum obs_source_render_route_draw)",
    "sizeof(enum obs_source_render_route_tech_end)",
    "sizeof(int)",
    "sizeof(bool)",
    "sizeof(uint32_t)",
];

unsafe extern "C" {
    /// `obs_source_render_route_filter()`.
    pub fn oracle_obs_source_render_route_filter(has_target: bool, has_parent: bool) -> u32;
    /// `obs_source_render_route_video_reads_flags()`.
    pub fn oracle_obs_source_render_route_video_reads_flags(source_type: c_int) -> u8;
    /// `obs_source_render_route_video_none()`.
    pub fn oracle_obs_source_render_route_video_none(source_type: c_int, flags: u32) -> u8;
    /// `obs_source_render_route_async_reads_flags()`.
    pub fn oracle_obs_source_render_route_async_reads_flags(source_type: c_int) -> u8;
    /// `obs_source_render_route_async_reads_rendering_filter()`.
    pub fn oracle_obs_source_render_route_async_reads_rendering_filter(
        source_type: c_int,
        flags: u32,
    ) -> u8;
    /// `obs_source_render_route_async_update()`.
    pub fn oracle_obs_source_render_route_async_update(
        source_type: c_int,
        flags: u32,
        rendering_filter: bool,
    ) -> u8;
    /// `obs_source_render_route_reads_enabled()`.
    pub fn oracle_obs_source_render_route_reads_enabled(has_data: bool) -> u8;
    /// `obs_source_render_route_inactive()`.
    pub fn oracle_obs_source_render_route_inactive(has_data: bool, enabled: bool) -> u8;
    /// `obs_source_render_route_draw()`.
    pub fn oracle_obs_source_render_route_draw(has_filters: bool) -> u32;
    /// `obs_source_render_route_draw_next()`.
    pub fn oracle_obs_source_render_route_draw_next(step: u32, value: bool) -> u32;
    /// `obs_source_render_route_main_custom_draw()`.
    pub fn oracle_obs_source_render_route_main_custom_draw(flags: u32) -> u8;
    /// `obs_source_render_route_main_srgb_aware()`.
    pub fn oracle_obs_source_render_route_main_srgb_aware(flags: u32) -> u8;
    /// `obs_source_render_route_main_reads_filters()`.
    pub fn oracle_obs_source_render_route_main_reads_filters(has_filter_parent: bool) -> u8;
    /// `obs_source_render_route_main_default_effect()`.
    pub fn oracle_obs_source_render_route_main_default_effect(
        flags: u32,
        has_filter_parent: bool,
        has_filters: bool,
    ) -> u8;
    /// `obs_source_render_route_bypass_reads_space()`.
    pub fn oracle_obs_source_render_route_bypass_reads_space(
        target_is_parent: bool,
        allow_direct: c_int,
        filter_flags: u32,
        parent_flags: u32,
    ) -> u8;
    /// `obs_source_render_route_bypass()`.
    pub fn oracle_obs_source_render_route_bypass(
        target_is_parent: bool,
        allow_direct: c_int,
        filter_flags: u32,
        parent_flags: u32,
        space: c_int,
        current_space: c_int,
    ) -> u8;
    /// `obs_source_render_route_size_skip()`.
    pub fn oracle_obs_source_render_route_size_skip(cx: c_int, cy: c_int) -> u8;
    /// `obs_source_render_route_texrender_stale()`.
    pub fn oracle_obs_source_render_route_texrender_stale(
        texrender_format: c_int,
        format: c_int,
    ) -> u8;
    /// `obs_source_render_route_filter_render()`.
    pub fn oracle_obs_source_render_route_filter_render(
        target_is_parent: bool,
        parent_flags: u32,
    ) -> u32;
    /// `obs_source_render_route_tech_end_linear_srgb()`.
    pub fn oracle_obs_source_render_route_tech_end_linear_srgb(filter_flags: u32) -> u8;
    /// `obs_source_render_route_tech_name()`.
    pub fn oracle_obs_source_render_route_tech_name(tech_name: *const c_char) -> *const c_char;
    /// `obs_source_render_route_tech_end()`.
    pub fn oracle_obs_source_render_route_tech_end(bypass_active: bool) -> u32;
    /// `obs_source_render_route_skip()`.
    pub fn oracle_obs_source_render_route_skip(target_is_parent: bool, parent_flags: u32) -> u32;
    /// The entry named `VALUE_NAMES[which]`; `i64::MIN` out of range.
    pub fn oracle_source_render_route_value(which: usize) -> i64;
}
