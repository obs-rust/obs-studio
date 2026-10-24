//! `oracle_*` build of `libobs/obs-source-transition-render.c.h`
//! (oracle/source_transition_render.c): the render and audio step decisions
//! of the transitions, renamed. Everything crosses as plain values, the
//! start frame through a `size_t *` and the offsets as a two-`float`
//! struct.

use core::ffi::c_int;

/// Index of each entry in `oracle_source_transition_render_layout`, in the
/// C's order.
pub const LAYOUT_NAMES: [&str; 27] = [
    "OBS_SOURCE_TRANSITION_RENDER_TARGET_B",
    "OBS_SOURCE_TRANSITION_RENDER_AUDIO_FRAMES",
    "sizeof(enum obs_source_transition_render_video_step)",
    "OBS_SOURCE_TRANSITION_RENDER_VIDEO_END",
    "OBS_SOURCE_TRANSITION_RENDER_VIDEO_STOP",
    "OBS_SOURCE_TRANSITION_RENDER_VIDEO_STOP_AUDIO",
    "sizeof(enum obs_source_transition_render_draw_step)",
    "OBS_SOURCE_TRANSITION_RENDER_DRAW_CALLBACK",
    "OBS_SOURCE_TRANSITION_RENDER_DRAW_B",
    "OBS_SOURCE_TRANSITION_RENDER_DRAW_A",
    "sizeof(enum obs_source_transition_render_audio_step)",
    "OBS_SOURCE_TRANSITION_RENDER_AUDIO_KEEP",
    "OBS_SOURCE_TRANSITION_RENDER_AUDIO_END",
    "OBS_SOURCE_TRANSITION_RENDER_AUDIO_STOP",
    "OBS_SOURCE_TRANSITION_RENDER_MIX_A",
    "OBS_SOURCE_TRANSITION_RENDER_MIX_B",
    "OBS_SOURCE_TRANSITION_RENDER_COPY_A",
    "sizeof(enum obs_source_transition_render_audio_place)",
    "OBS_SOURCE_TRANSITION_RENDER_PLACE_NONE",
    "OBS_SOURCE_TRANSITION_RENDER_PLACE_LATE",
    "OBS_SOURCE_TRANSITION_RENDER_PLACE_MIX",
    "sizeof(struct obs_source_transition_render_offset)",
    "offsetof(struct obs_source_transition_render_offset, y)",
    "sizeof(int)",
    "sizeof(bool)",
    "sizeof(size_t)",
    "FLT_EVAL_METHOD",
];

/// `struct obs_source_transition_render_offset`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct OracleOffset {
    /// `x`
    pub x: f32,
    /// `y`
    pub y: f32,
}

unsafe extern "C" {
    /// `obs_source_transition_render_video_due`.
    pub fn oracle_obs_source_transition_render_video_due(t: f32, transitioning_video: bool)
    -> bool;
    /// `obs_source_transition_render_video_stop`: the step as the C's enum,
    /// an `int` or `unsigned int` (every value fits both).
    pub fn oracle_obs_source_transition_render_video_stop(
        transitioning_audio: bool,
        active: bool,
    ) -> u32;
    /// `obs_source_transition_render_draw`: as the C's enum.
    pub fn oracle_obs_source_transition_render_draw(
        transitioning_video: bool,
        textures_locked: bool,
        has_callback: bool,
        transitioning_audio: bool,
    ) -> u32;
    /// `obs_source_transition_render_direct_index`.
    pub fn oracle_obs_source_transition_render_direct_index(
        transitioning: bool,
        target: c_int,
    ) -> c_int;
    /// `obs_source_transition_render_audio_stop`: as the C's enum.
    pub fn oracle_obs_source_transition_render_audio_stop(
        has_min_ts: bool,
        t: f32,
        transitioning_video: bool,
        transitioning_audio: bool,
    ) -> u32;
    /// `obs_source_transition_render_audio_mix`: the bits.
    pub fn oracle_obs_source_transition_render_audio_mix(
        transitioning_audio: bool,
        has_a: bool,
        has_b: bool,
    ) -> u32;
    /// `obs_source_transition_render_audio_valid`.
    pub fn oracle_obs_source_transition_render_audio_valid(
        present: bool,
        pending: bool,
        duplicated: bool,
    ) -> bool;
    /// `obs_source_transition_render_audio_place`: writes `*pos` (valid)
    /// only for a late or mixed child; as the C's enum.
    pub fn oracle_obs_source_transition_render_audio_place(
        ts: u64,
        min_ts: u64,
        sample_rate: usize,
        pos: *mut usize,
    ) -> u32;
    /// `obs_source_transition_render_offset`.
    pub fn oracle_obs_source_transition_render_offset(
        tr_cx: f32,
        tr_cy: f32,
        source_cx: f32,
        source_cy: f32,
        scale_x: f32,
        scale_y: f32,
    ) -> OracleOffset;
    /// The entry named `LAYOUT_NAMES[which]`; 0 out of range.
    pub fn oracle_source_transition_render_layout(which: usize) -> usize;
}
