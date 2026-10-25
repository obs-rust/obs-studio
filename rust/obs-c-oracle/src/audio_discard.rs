//! `oracle_*` build of `libobs/obs-audio-discard.c.h`
//! (oracle/audio_discard.c).
//!
//! The structs cross as untyped pointers: callers lay them out themselves
//! (`oracle_audio_discard_layout` says how the C sees them), as the shims'
//! `#[repr(C)]` mirrors do. The steps come back by value as
//! [`OracleAudioIgnoreStep`] and [`OracleAudioDiscardStep`]; the C enums
//! are `int`.

use core::ffi::c_void;

/// Index of each entry in `oracle_audio_discard_layout`, in the C's order.
pub const LAYOUT_NAMES: [&str; 31] = [
    "sizeof(struct obs_audio_input)",
    "alignof(struct obs_audio_input)",
    "offsetof(struct obs_audio_input, buf)",
    "offsetof(struct obs_audio_input, audio_ts)",
    "offsetof(struct obs_audio_input, last_size)",
    "offsetof(struct obs_audio_input, pending)",
    "offsetof(struct obs_audio_input, pending_stop)",
    "offsetof(struct obs_audio_input, timing_set)",
    "sizeof(struct obs_audio_ignore_step)",
    "alignof(struct obs_audio_ignore_step)",
    "offsetof(struct obs_audio_ignore_step, rerender)",
    "offsetof(struct obs_audio_ignore_step, no_timestamp)",
    "offsetof(struct obs_audio_ignore_step, buffered)",
    "offsetof(struct obs_audio_ignore_step, dropped)",
    "offsetof(struct obs_audio_ignore_step, lag)",
    "offsetof(struct obs_audio_ignore_step, warn)",
    "offsetof(struct obs_audio_ignore_step, warn_lag)",
    "sizeof(struct obs_audio_discard_step)",
    "alignof(struct obs_audio_discard_step)",
    "offsetof(struct obs_audio_discard_step, path)",
    "offsetof(struct obs_audio_discard_step, stop)",
    "sizeof(enum obs_audio_stop_check)",
    "sizeof(enum obs_audio_discard_path)",
    "OBS_AUDIO_STOP_NONE",
    "OBS_AUDIO_STOP_PENDING",
    "OBS_AUDIO_STOP_CLEARED",
    "OBS_AUDIO_DISCARD_RENDER_SOURCE",
    "OBS_AUDIO_DISCARD_AHEAD",
    "OBS_AUDIO_DISCARD_BEHIND",
    "OBS_AUDIO_DISCARD_START_AT_END",
    "OBS_AUDIO_DISCARD_PENDING",
];

/// `struct obs_audio_ignore_step`, as the oracle returns it.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OracleAudioIgnoreStep {
    pub rerender: bool,
    pub no_timestamp: bool,
    pub buffered: usize,
    pub dropped: usize,
    pub lag: u64,
    pub warn: bool,
    pub warn_lag: u64,
}

/// `struct obs_audio_discard_step`, as the oracle returns it.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OracleAudioDiscardStep {
    pub path: i32,
    pub stop: i32,
}

unsafe extern "C" {
    /// `input`: `const struct obs_audio_input *`.
    pub fn oracle_obs_audio_discard_ignore(
        input: *const c_void,
        channels: usize,
        sample_rate: usize,
        start_ts: u64,
    ) -> OracleAudioIgnoreStep;
    /// `input`: `const struct obs_audio_input *`; returns the `int` of
    /// `enum obs_audio_stop_check`.
    pub fn oracle_obs_audio_discard_if_stopped(input: *const c_void, channels: usize) -> i32;
    /// `input`: `const struct obs_audio_input *`; `ts`: `const struct
    /// ts_info *`.
    pub fn oracle_obs_audio_discard_tick(
        input: *const c_void,
        audio_render: bool,
        channels: usize,
        sample_rate: usize,
        ts: *const c_void,
    ) -> OracleAudioDiscardStep;
    /// The entry named `LAYOUT_NAMES[which]`; 0 out of range.
    pub fn oracle_audio_discard_layout(which: usize) -> usize;
}
