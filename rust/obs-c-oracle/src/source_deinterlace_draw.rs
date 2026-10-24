//! `oracle_*` build of `libobs/obs-source-deinterlace-draw.c.h`
//! (oracle/source_deinterlace_draw.c): the deinterlace draw decisions,
//! renamed. The `bool` results are declared as `u8`, the byte the C
//! returns, so a parity test compares it without assuming it is 0 or 1;
//! the enums as `i32`, their value.

use std::os::raw::c_int;

/// Index of each entry in `oracle_source_deinterlace_draw_value`, in the
/// C's order.
pub const VALUE_NAMES: [&str; 39] = [
    "OBS_DEINTERLACE_MODE_DISABLE",
    "OBS_DEINTERLACE_MODE_DISCARD",
    "OBS_DEINTERLACE_MODE_RETRO",
    "OBS_DEINTERLACE_MODE_BLEND",
    "OBS_DEINTERLACE_MODE_BLEND_2X",
    "OBS_DEINTERLACE_MODE_LINEAR",
    "OBS_DEINTERLACE_MODE_LINEAR_2X",
    "OBS_DEINTERLACE_MODE_YADIF",
    "OBS_DEINTERLACE_MODE_YADIF_2X",
    "GS_CS_SRGB",
    "GS_CS_SRGB_16F",
    "GS_CS_709_EXTENDED",
    "GS_CS_709_SCRGB",
    "OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_NONE",
    "OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_DISCARD",
    "OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_DISCARD_2X",
    "OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_BLEND",
    "OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_BLEND_2X",
    "OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_LINEAR",
    "OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_LINEAR_2X",
    "OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_YADIF",
    "OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_YADIF_2X",
    "OBS_SOURCE_DRAW_TECH_DRAW",
    "OBS_SOURCE_DRAW_TECH_DRAW_MULTIPLY",
    "OBS_SOURCE_DRAW_TECH_DRAW_TONEMAP",
    "OBS_SOURCE_DRAW_TECH_DRAW_MULTIPLY_TONEMAP",
    "OBS_SOURCE_DEINTERLACE_DRAW_MODE_SAME",
    "OBS_SOURCE_DEINTERLACE_DRAW_MODE_ENABLE",
    "OBS_SOURCE_DEINTERLACE_DRAW_MODE_DISABLE",
    "OBS_SOURCE_DEINTERLACE_DRAW_MODE_SWITCH",
    "OBS_SOURCE_DEINTERLACE_PACING_TWOX_TOLERANCE",
    "sizeof(enum obs_deinterlace_mode)",
    "sizeof(enum gs_color_space)",
    "sizeof(enum obs_source_deinterlace_draw_effect)",
    "sizeof(enum obs_source_draw_technique)",
    "sizeof(enum obs_source_deinterlace_draw_mode_change)",
    "sizeof(int)",
    "sizeof(bool)",
    "sizeof(float)",
];

unsafe extern "C" {
    /// `enum obs_source_deinterlace_draw_effect
    /// obs_source_deinterlace_draw_effect(int mode)`.
    pub fn oracle_obs_source_deinterlace_draw_effect(mode: c_int) -> c_int;
    /// `bool obs_source_deinterlace_draw_linear_required(int mode)`.
    pub fn oracle_obs_source_deinterlace_draw_linear_required(mode: c_int) -> u8;
    /// `bool obs_source_deinterlace_draw_skip(...)`.
    pub fn oracle_obs_source_deinterlace_draw_skip(
        has_cur_tex: bool,
        has_prev_tex: bool,
        width: u32,
        height: u32,
    ) -> u8;
    /// `bool obs_source_deinterlace_draw_reads_linear_srgb(int source_space)`.
    pub fn oracle_obs_source_deinterlace_draw_reads_linear_srgb(source_space: c_int) -> u8;
    /// `bool obs_source_deinterlace_draw_linear_srgb(...)`.
    pub fn oracle_obs_source_deinterlace_draw_linear_srgb(
        source_space: c_int,
        gs_linear_srgb: bool,
        mode: c_int,
    ) -> u8;
    /// `enum obs_source_deinterlace_draw_technique
    /// obs_source_deinterlace_draw_technique(...)`.
    pub fn oracle_obs_source_deinterlace_draw_technique(
        source_space: c_int,
        current_space: c_int,
    ) -> u32;
    /// `bool obs_source_deinterlace_draw_reads_white_level(...)`.
    pub fn oracle_obs_source_deinterlace_draw_reads_white_level(
        source_space: c_int,
        current_space: c_int,
    ) -> u8;
    /// `float obs_source_deinterlace_draw_multiplier(...)`.
    pub fn oracle_obs_source_deinterlace_draw_multiplier(
        source_space: c_int,
        current_space: c_int,
        sdr_white_level: f32,
    ) -> f32;
    /// `bool obs_source_deinterlace_draw_frame2(...)`.
    pub fn oracle_obs_source_deinterlace_draw_frame2(
        video_time: u64,
        frame_ts: u64,
        offset: u64,
        half_duration: u32,
    ) -> u8;
    /// `enum obs_source_deinterlace_draw_mode_change
    /// obs_source_deinterlace_draw_mode_change(int current_mode, int mode)`.
    pub fn oracle_obs_source_deinterlace_draw_mode_change(current_mode: c_int, mode: c_int) -> u32;
    /// The entry named `VALUE_NAMES[which]`; `i64::MIN` out of range.
    pub fn oracle_source_deinterlace_draw_value(which: usize) -> i64;
}
