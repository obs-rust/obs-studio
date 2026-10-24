//! `oracle_*` build of `libobs/obs-source-draw-plan.c.h`
//! (oracle/source_draw_plan.c): the source draw decisions, renamed. The
//! `bool` results are declared as `u8`, the byte the C returns, so a parity
//! test compares it without assuming it is 0 or 1; the technique as `u32`,
//! its value.

use std::os::raw::{c_char, c_int, c_long};

/// Index of each entry in `oracle_source_draw_plan_value`, in the C's
/// order.
pub const VALUE_NAMES: [&str; 16] = [
    "GS_CS_SRGB",
    "GS_CS_SRGB_16F",
    "GS_CS_709_EXTENDED",
    "GS_CS_709_SCRGB",
    "OBS_SOURCE_DRAW_TECH_DRAW",
    "OBS_SOURCE_DRAW_TECH_DRAW_MULTIPLY",
    "OBS_SOURCE_DRAW_TECH_DRAW_TONEMAP",
    "OBS_SOURCE_DRAW_TECH_DRAW_MULTIPLY_TONEMAP",
    "OBS_SOURCE_DRAW_TECH_DRAW_NONLINEAR_ALPHA",
    "OBS_SOURCE_DRAW_TECH_DRAW_NONLINEAR_ALPHA_MULTIPLY",
    "sizeof(enum gs_color_space)",
    "sizeof(enum obs_source_draw_technique)",
    "sizeof(int)",
    "sizeof(long)",
    "sizeof(bool)",
    "sizeof(float)",
];

unsafe extern "C" {
    /// `float obs_source_draw_plan_rotation_x(long rotation, uint32_t width,
    /// uint32_t height)`.
    pub fn oracle_obs_source_draw_plan_rotation_x(rotation: c_long, width: u32, height: u32)
    -> f32;
    /// `float obs_source_draw_plan_rotation_y(...)`.
    pub fn oracle_obs_source_draw_plan_rotation_y(rotation: c_long, width: u32, height: u32)
    -> f32;
    /// `bool obs_source_draw_plan_async_linear_srgb(...)`.
    pub fn oracle_obs_source_draw_plan_async_linear_srgb(
        source_space: c_int,
        current_space: c_int,
        gs_linear_srgb: bool,
    ) -> u8;
    /// `bool obs_source_draw_plan_async_nonlinear_alpha(...)`.
    pub fn oracle_obs_source_draw_plan_async_nonlinear_alpha(
        source_space: c_int,
        linear_srgb: bool,
        linear_alpha: bool,
    ) -> u8;
    /// `enum obs_source_draw_technique obs_source_draw_plan_technique(...)`.
    pub fn oracle_obs_source_draw_plan_technique(
        source_space: c_int,
        current_space: c_int,
        nonlinear_alpha: bool,
    ) -> u32;
    /// `bool obs_source_draw_plan_reads_white_level(...)`.
    pub fn oracle_obs_source_draw_plan_reads_white_level(
        source_space: c_int,
        current_space: c_int,
    ) -> u8;
    /// `float obs_source_draw_plan_multiplier(...)`.
    pub fn oracle_obs_source_draw_plan_multiplier(
        source_space: c_int,
        current_space: c_int,
        sdr_white_level: f32,
    ) -> f32;
    /// `bool obs_source_draw_plan_convert(...)`.
    pub fn oracle_obs_source_draw_plan_convert(source_space: c_int, current_space: c_int) -> u8;
    /// `obs_source_draw_technique_name()` for a technique value.
    pub fn oracle_source_draw_plan_technique_name(technique: c_int) -> *const c_char;
    /// The entry named `VALUE_NAMES[which]`; `i64::MIN` out of range.
    pub fn oracle_source_draw_plan_value(which: usize) -> i64;
}
