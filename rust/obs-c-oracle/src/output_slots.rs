//! `oracle_*` build of `libobs/obs-output-slots.c.h`
//! (oracle/output_slots.c): the encoder slot rules and setter gates,
//! renamed.
//!
//! Every function takes and returns plain values, except the index the two
//! `first` functions write; nothing here divides or calls out.
//! `oracle_obs_output_slots_mixer_mask` is the C's shift by `mixer_idx`:
//! callers keep it below the width of `usize`, where the C is defined.

use std::os::raw::c_int;

/// Index of each entry in `oracle_output_slots_constant`, in the C's order.
pub const CONSTANT_NAMES: [&str; 7] = [
    "OBS_OUTPUT_SLOTS_MULTI_TRACK_AUDIO",
    "OBS_OUTPUT_SLOTS_MULTI_TRACK_VIDEO",
    "OBS_OUTPUT_SLOTS_MAX_VIDEO_ENCODERS",
    "OBS_OUTPUT_SLOTS_MAX_AUDIO_ENCODERS",
    "OBS_OUTPUT_SLOTS_MAX_AUDIO_MIXES",
    "OBS_OUTPUT_SLOTS_ENCODER_AUDIO",
    "OBS_OUTPUT_SLOTS_ENCODER_VIDEO",
];

unsafe extern "C" {
    /// Writes `*index` only when a slot is set.
    pub fn oracle_obs_output_slots_first_video(occupied: u32, index: *mut usize) -> bool;

    /// Writes `*index` only when a slot is set.
    pub fn oracle_obs_output_slots_first_audio(occupied: u32, index: *mut usize) -> bool;

    pub fn oracle_obs_output_slots_mixer_in_range(mixer_idx: usize) -> bool;

    /// The C shifts by `mixer_idx`: undefined from the width of `usize` up.
    pub fn oracle_obs_output_slots_mixer_mask(mixer_idx: usize) -> usize;

    pub fn oracle_obs_output_slots_video_type_ok(encoder_type: c_int) -> bool;

    pub fn oracle_obs_output_slots_audio_type_ok(encoder_type: c_int) -> bool;

    pub fn oracle_obs_output_slots_video_index_ok(flags: u32, idx: usize) -> bool;

    pub fn oracle_obs_output_slots_audio_index_ok(flags: u32, idx: usize) -> bool;

    pub fn oracle_obs_output_slots_default_index(idx: usize) -> bool;

    pub fn oracle_obs_output_slots_scaled_axis_set(size: u32) -> bool;

    pub fn oracle_obs_output_slots_video_slot(idx: usize) -> bool;

    pub fn oracle_obs_output_slots_audio_slot(idx: usize) -> bool;

    pub fn oracle_output_slots_constant(which: usize) -> i64;
}
