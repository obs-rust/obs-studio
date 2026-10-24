//! `oracle_*` build of `libobs/obs-scene-audio-window.c.h`
//! (oracle/scene_audio_window.c).
//!
//! The window comes back by value as [`OracleAudioWindow`], the layout of
//! `struct obs_scene_audio_window`; the child as the C's enum value.

use std::ffi::c_int;

/// `struct obs_scene_audio_window`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OracleAudioWindow {
    pub mix: bool,
    pub pos: usize,
    pub count: usize,
}

/// Index of each entry in `oracle_scene_audio_window_layout`, in the C's
/// order.
pub const LAYOUT_NAMES: [&str; 10] = [
    "sizeof(enum obs_scene_audio_window_child)",
    "OBS_SCENE_AUDIO_WINDOW_SOURCE",
    "OBS_SCENE_AUDIO_WINDOW_SHOW",
    "OBS_SCENE_AUDIO_WINDOW_HIDE",
    "sizeof(struct obs_scene_audio_window)",
    "alignof(struct obs_scene_audio_window)",
    "offsetof(struct obs_scene_audio_window, mix)",
    "offsetof(struct obs_scene_audio_window, pos)",
    "offsetof(struct obs_scene_audio_window, count)",
    "AUDIO_OUTPUT_FRAMES",
];

unsafe extern "C" {
    pub fn oracle_obs_scene_audio_window_pick(
        visible: bool,
        show_active: bool,
        hide_active: bool,
    ) -> c_int;
    pub fn oracle_obs_scene_audio_window_counts(
        pending: bool,
        visible: bool,
        hide_active: bool,
    ) -> bool;
    pub fn oracle_obs_scene_audio_window_earliest(timestamp: u64, source_ts: u64) -> u64;
    pub fn oracle_obs_scene_audio_window_silent(timestamp: u64) -> bool;
    pub fn oracle_obs_scene_audio_window_frames(
        pending: bool,
        source_ts: u64,
        timestamp: u64,
        sample_rate: usize,
    ) -> OracleAudioWindow;
    pub fn oracle_obs_scene_audio_window_mixes(
        apply_buf: bool,
        visible: bool,
        hide_active: bool,
    ) -> bool;
    pub fn oracle_obs_scene_audio_window_adds(audio_is_duplicated: bool) -> bool;
    /// The entry named `LAYOUT_NAMES[which]`; 0 out of range.
    pub fn oracle_scene_audio_window_layout(which: usize) -> usize;
}
