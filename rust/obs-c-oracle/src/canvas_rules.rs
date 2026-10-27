//! `oracle_*` build of `libobs/obs-canvas-rules.c.h` (oracle/canvas_rules.c):
//! the canvas flag rules and setter checks, renamed. The `bool` results are
//! declared as `u8`, the byte the C returns, so a parity test compares it
//! without assuming it is 0 or 1; the flags as `u32`, the format as
//! [`c_int`].

use std::os::raw::{c_char, c_int};

/// Index of each entry in `oracle_canvas_rules_value`, in the C's order.
pub const VALUE_NAMES: [&str; 23] = [
    "OBS_CANVAS_RULES_MAIN",
    "OBS_CANVAS_RULES_ACTIVATE",
    "OBS_CANVAS_RULES_MIX_AUDIO",
    "OBS_CANVAS_RULES_SCENE_REF",
    "OBS_CANVAS_RULES_EPHEMERAL",
    "OBS_CANVAS_RULES_REMOVED",
    "OBS_CANVAS_RULES_PUBLIC",
    "OBS_CANVAS_RULES_FORMAT_NONE",
    "OBS_CANVAS_RULES_MAX_CHANNELS",
    "MAIN",
    "ACTIVATE",
    "MIX_AUDIO",
    "SCENE_REF",
    "EPHEMERAL",
    "PROGRAM",
    "PREVIEW",
    "DEVICE",
    "VIDEO_FORMAT_NONE",
    "MAX_CHANNELS",
    "sizeof(enum video_format)",
    "sizeof(int)",
    "sizeof(bool)",
    "sizeof(uint32_t)",
];

unsafe extern "C" {
    /// `uint32_t obs_canvas_rules_main_flags(void)`.
    pub fn oracle_obs_canvas_rules_main_flags() -> u32;
    /// `uint32_t obs_canvas_rules_public_flags(uint32_t flags)`.
    pub fn oracle_obs_canvas_rules_public_flags(flags: u32) -> u32;
    /// `bool obs_canvas_rules_main_view(uint32_t flags)`.
    pub fn oracle_obs_canvas_rules_main_view(flags: u32) -> u8;
    /// `bool obs_canvas_rules_mix_audio(uint32_t flags)`.
    pub fn oracle_obs_canvas_rules_mix_audio(flags: u32) -> u8;
    /// `bool obs_canvas_rules_public(bool is_private)`.
    pub fn oracle_obs_canvas_rules_public(is_private: bool) -> u8;
    /// `bool obs_canvas_rules_holds_scenes(uint32_t flags)`.
    pub fn oracle_obs_canvas_rules_holds_scenes(flags: u32) -> u8;
    /// `bool obs_canvas_rules_saves(uint32_t flags)`.
    pub fn oracle_obs_canvas_rules_saves(flags: u32) -> u8;
    /// `bool obs_canvas_rules_size_set(uint32_t size)`.
    pub fn oracle_obs_canvas_rules_size_set(size: u32) -> u8;
    /// `bool obs_canvas_rules_format_set(int format)`.
    pub fn oracle_obs_canvas_rules_format_set(format: c_int) -> u8;
    /// `bool obs_canvas_rules_resets(uint32_t flags)`.
    pub fn oracle_obs_canvas_rules_resets(flags: u32) -> u8;
    /// `bool obs_canvas_rules_global_signals(uint32_t flags)`.
    pub fn oracle_obs_canvas_rules_global_signals(flags: u32) -> u8;
    /// `bool obs_canvas_rules_channel_ok(uint32_t channel)`.
    pub fn oracle_obs_canvas_rules_channel_ok(channel: u32) -> u8;
    /// `bool obs_canvas_rules_name_empty(const char *name)`.
    pub fn oracle_obs_canvas_rules_name_empty(name: *const c_char) -> u8;
    /// `bool obs_canvas_rules_renames(uint32_t flags)`.
    pub fn oracle_obs_canvas_rules_renames(flags: u32) -> u8;
    /// `bool obs_canvas_rules_removable(uint32_t flags)`.
    pub fn oracle_obs_canvas_rules_removable(flags: u32) -> u8;
    /// `uint32_t obs_canvas_rules_mark_removed(uint32_t flags)`.
    pub fn oracle_obs_canvas_rules_mark_removed(flags: u32) -> u32;
    /// `bool obs_canvas_rules_removed(uint32_t flags)`.
    pub fn oracle_obs_canvas_rules_removed(flags: u32) -> u8;
    /// The entry named `VALUE_NAMES[which]`; `i64::MIN` out of range.
    pub fn oracle_canvas_rules_value(which: usize) -> i64;
}
