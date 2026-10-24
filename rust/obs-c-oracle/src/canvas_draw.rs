//! `oracle_*` build of `libobs/obs-canvas-draw.c.h` (oracle/canvas_draw.c):
//! the canvas texture draw and display clear decisions, renamed. The `bool`
//! results are declared as `u8`, the byte the C returns, so a parity test
//! compares it without assuming it is 0 or 1; the technique as `u32`, its
//! value.

use std::os::raw::{c_char, c_int};

/// Index of each entry in `oracle_canvas_draw_value`, in the C's order.
pub const VALUE_NAMES: [&str; 19] = [
    "GS_CS_SRGB",
    "GS_CS_SRGB_16F",
    "GS_CS_709_EXTENDED",
    "GS_CS_709_SCRGB",
    "OBS_SOURCE_DRAW_TECH_DRAW",
    "OBS_SOURCE_DRAW_TECH_DRAW_MULTIPLY",
    "OBS_SOURCE_DRAW_TECH_DRAW_TONEMAP",
    "OBS_DISPLAY_CLEAR_COLOR",
    "OBS_DISPLAY_CLEAR_DEPTH",
    "OBS_DISPLAY_CLEAR_STENCIL",
    "GS_CLEAR_COLOR",
    "GS_CLEAR_DEPTH",
    "GS_CLEAR_STENCIL",
    "sizeof(enum gs_color_space)",
    "sizeof(enum obs_source_draw_technique)",
    "sizeof(int)",
    "sizeof(bool)",
    "sizeof(float)",
    "sizeof(uint32_t)",
];

unsafe extern "C" {
    /// `bool obs_canvas_draw_skip(bool has_mix, bool texture_rendered)`.
    pub fn oracle_obs_canvas_draw_skip(has_mix: bool, texture_rendered: bool) -> u8;
    /// `enum obs_source_draw_technique obs_canvas_draw_technique(int
    /// source_space, int current_space)`.
    pub fn oracle_obs_canvas_draw_technique(source_space: c_int, current_space: c_int) -> u32;
    /// `bool obs_canvas_draw_reads_white_level(int current_space)`.
    pub fn oracle_obs_canvas_draw_reads_white_level(current_space: c_int) -> u8;
    /// `float obs_canvas_draw_multiplier(float sdr_white_level)`.
    pub fn oracle_obs_canvas_draw_multiplier(sdr_white_level: f32) -> f32;
    /// `bool obs_display_clear_reads_space(bool always_srgb)`.
    pub fn oracle_obs_display_clear_reads_space(always_srgb: bool) -> u8;
    /// `bool obs_display_clear_linear(int current_space)`.
    pub fn oracle_obs_display_clear_linear(current_space: c_int) -> u8;
    /// `uint32_t obs_display_clear_flags(bool use_clear_workaround)`.
    pub fn oracle_obs_display_clear_flags(use_clear_workaround: bool) -> u32;
    /// `bool obs_display_clear_draws_solid(bool use_clear_workaround)`.
    pub fn oracle_obs_display_clear_draws_solid(use_clear_workaround: bool) -> u8;
    /// `obs_source_draw_technique_name()` for a technique value.
    pub fn oracle_canvas_draw_technique_name(technique: c_int) -> *const c_char;
    /// The entry named `VALUE_NAMES[which]`; `i64::MIN` out of range.
    pub fn oracle_canvas_draw_value(which: usize) -> i64;
}
