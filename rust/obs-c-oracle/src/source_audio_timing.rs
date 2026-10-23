//! `oracle_*` build of `libobs/obs-source-audio-timing.c.h`
//! (oracle/source_audio_timing.c): the source audio timestamp placement,
//! renamed. The structs are passed as raw bytes of the C layout, so a
//! parity test can compare every byte the two sides leave.

use core::ffi::c_void;

/// Index of each entry in `oracle_source_audio_timing_layout`, in the C's
/// order.
pub const LAYOUT_NAMES: [&str; 39] = [
    "sizeof(struct obs_source_audio_timing_pre)",
    "alignof(struct obs_source_audio_timing_pre)",
    "offsetof(pre, timestamp)",
    "offsetof(pre, os_time)",
    "offsetof(pre, duration)",
    "offsetof(pre, next_audio_ts_min)",
    "offsetof(pre, timing_adjust)",
    "offsetof(pre, diff)",
    "offsetof(pre, drift_error)",
    "offsetof(pre, action)",
    "offsetof(pre, timing_set)",
    "offsetof(pre, async_unbuffered)",
    "offsetof(pre, async_decoupled)",
    "offsetof(pre, direct)",
    "offsetof(pre, reanchor)",
    "offsetof(pre, continuous)",
    "sizeof(struct obs_source_audio_timing_under)",
    "alignof(struct obs_source_audio_timing_under)",
    "offsetof(under, timestamp)",
    "offsetof(under, packet_timing_adjust)",
    "offsetof(under, data_timestamp)",
    "offsetof(under, os_time)",
    "offsetof(under, next_audio_ts_min)",
    "offsetof(under, resample_offset)",
    "offsetof(under, sync_offset)",
    "offsetof(under, timing_adjust)",
    "offsetof(under, next_audio_sys_ts_min)",
    "offsetof(under, last_sync_offset)",
    "offsetof(under, continuous)",
    "offsetof(under, push_back)",
    "offsetof(under, reset_timing)",
    "sizeof(enum obs_source_audio_ts_action)",
    "OBS_SOURCE_AUDIO_TS_KEEP",
    "OBS_SOURCE_AUDIO_TS_RESET",
    "OBS_SOURCE_AUDIO_TS_JUMP",
    "OBS_SOURCE_AUDIO_TS_SMOOTH",
    "OBS_SOURCE_AUDIO_TS_EXCEEDED",
    "OBS_SOURCE_AUDIO_TS_SMOOTHING_THRESHOLD",
    "OBS_SOURCE_AUDIO_MAX_TS_VAR",
];

unsafe extern "C" {
    /// `obs_source_audio_timing_pre_lock`: `t` points to a
    /// `struct obs_source_audio_timing_pre`.
    pub fn oracle_obs_source_audio_timing_pre_lock(t: *mut c_void);
    /// `obs_source_audio_timing_under_lock`: `t` points to a
    /// `struct obs_source_audio_timing_under`.
    pub fn oracle_obs_source_audio_timing_under_lock(t: *mut c_void);
    pub fn oracle_obs_source_audio_timing_buffer_start(
        os_time: u64,
        sync_offset: i64,
        resample_offset: u64,
    ) -> u64;
    /// The entry named `LAYOUT_NAMES[which]`; 0 out of range.
    pub fn oracle_source_audio_timing_layout(which: usize) -> usize;
}
