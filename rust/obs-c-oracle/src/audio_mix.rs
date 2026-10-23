//! `oracle_*` build of `libobs/obs-audio-mix.c.h` (oracle/audio_mix.c).
//!
//! The structs cross as untyped pointers: callers lay them out themselves
//! (`oracle_audio_mix_layout` says how the C sees them), as the shims'
//! `#[repr(C)]` mirrors do. The step comes back by value as an
//! [`OracleAudioBufferingStep`].

use core::ffi::c_void;

/// Index of each entry in `oracle_audio_mix_layout`, in the C's order.
pub const LAYOUT_NAMES: [&str; 27] = [
    "sizeof(struct ts_info)",
    "alignof(struct ts_info)",
    "offsetof(struct ts_info, start)",
    "offsetof(struct ts_info, end)",
    "sizeof(struct audio_output_data)",
    "alignof(struct audio_output_data)",
    "offsetof(struct audio_output_data, data)",
    "sizeof(struct obs_audio_buffering)",
    "alignof(struct obs_audio_buffering)",
    "offsetof(struct obs_audio_buffering, buffered_ts)",
    "offsetof(struct obs_audio_buffering, buffered_timestamps)",
    "offsetof(struct obs_audio_buffering, buffering_wait_ticks)",
    "offsetof(struct obs_audio_buffering, total_buffering_ticks)",
    "offsetof(struct obs_audio_buffering, max_buffering_ticks)",
    "sizeof(struct obs_audio_buffering_step)",
    "alignof(struct obs_audio_buffering_step)",
    "offsetof(struct obs_audio_buffering_step, added)",
    "offsetof(struct obs_audio_buffering_step, max_reached)",
    "offsetof(struct obs_audio_buffering_step, ticks)",
    "offsetof(struct obs_audio_buffering_step, ms)",
    "offsetof(struct obs_audio_buffering_step, total_ms)",
    "sizeof(struct deque)",
    "alignof(struct deque)",
    "AUDIO_OUTPUT_FRAMES",
    "MAX_AUDIO_MIXES",
    "MAX_AUDIO_CHANNELS",
    "sizeof(int)",
];

/// `struct obs_audio_buffering_step`, as the oracle returns it.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OracleAudioBufferingStep {
    pub added: bool,
    pub max_reached: bool,
    pub ticks: i32,
    pub ms: usize,
    pub total_ms: usize,
}

unsafe extern "C" {
    pub fn oracle_obs_audio_mix_time_to_frames(sample_rate: usize, t: u64) -> usize;
    /// `mixes`: `struct audio_output_data[MAX_AUDIO_MIXES]`; `buf`: the
    /// source's `float *[MAX_AUDIO_MIXES][MAX_AUDIO_CHANNELS]`; `ts`:
    /// `const struct ts_info *`.
    pub fn oracle_obs_audio_mix_add(
        mixes: *mut c_void,
        buf: *mut c_void,
        audio_ts: u64,
        ts: *const c_void,
        channels: usize,
        sample_rate: usize,
    );
    /// `buffering`: `const struct obs_audio_buffering *`.
    pub fn oracle_obs_audio_buffering_maxed(buffering: *const c_void) -> bool;
    /// `buffering`: `const struct obs_audio_buffering *`; `ts`:
    /// `struct ts_info *`.
    pub fn oracle_obs_audio_buffering_fixed(
        buffering: *const c_void,
        sample_rate: usize,
        ts: *mut c_void,
    ) -> OracleAudioBufferingStep;
    /// As [`oracle_obs_audio_buffering_fixed`], plus `min_ts`.
    pub fn oracle_obs_audio_buffering_add(
        buffering: *const c_void,
        sample_rate: usize,
        ts: *mut c_void,
        min_ts: u64,
    ) -> OracleAudioBufferingStep;
    /// The entry named `LAYOUT_NAMES[which]`; 0 out of range.
    pub fn oracle_audio_mix_layout(which: usize) -> usize;
}
