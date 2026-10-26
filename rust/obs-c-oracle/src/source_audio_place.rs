//! `oracle_*` build of `libobs/obs-source-audio-place.c.h`
//! (oracle/source_audio_place.c): the source audio placement and action
//! timeline of `source_output_audio_place()`,
//! `source_output_audio_push_back()`, `source_output_audio_data()` and
//! `apply_audio_actions()`, renamed. The `bool` results are declared as
//! `u8`, the byte the C returns, so a parity test compares it without
//! assuming it is 0 or 1; `size_t` as `usize`, the monitoring type as
//! [`c_int`].

use std::os::raw::c_int;

/// Index of each entry in `oracle_source_audio_place_value`, in the C's
/// order.
pub const VALUE_NAMES: [&str; 4] = [
    "OBS_SOURCE_AUDIO_PLACE_FRAMES",
    "OBS_SOURCE_AUDIO_PLACE_MAX_BUF_SIZE",
    "OBS_SOURCE_AUDIO_PLACE_MONITOR_ONLY",
    "sizeof(size_t)",
];

unsafe extern "C" {
    /// `bool obs_source_audio_place_started(uint64_t audio_ts)`.
    pub fn oracle_obs_source_audio_place_started(audio_ts: u64) -> u8;
    /// `bool obs_source_audio_place_older(uint64_t timestamp, uint64_t
    /// audio_ts)`.
    pub fn oracle_obs_source_audio_place_older(timestamp: u64, audio_ts: u64) -> u8;
    /// `size_t obs_source_audio_place_size(uint32_t frames)`.
    pub fn oracle_obs_source_audio_place_size(frames: u32) -> usize;
    /// `size_t obs_source_audio_place_offset(uint64_t timestamp, uint64_t
    /// audio_ts, uint32_t sample_rate)`.
    pub fn oracle_obs_source_audio_place_offset(
        timestamp: u64,
        audio_ts: u64,
        sample_rate: u32,
    ) -> usize;
    /// `bool obs_source_audio_place_fits(size_t start, size_t size)`.
    pub fn oracle_obs_source_audio_place_fits(start: usize, size: usize) -> u8;
    /// `size_t obs_source_audio_place_pop_back(size_t buf_size, size_t
    /// placement, size_t size)`.
    pub fn oracle_obs_source_audio_place_pop_back(
        buf_size: usize,
        placement: usize,
        size: usize,
    ) -> usize;
    /// `bool obs_source_audio_place_outputs(int monitoring_type)`.
    pub fn oracle_obs_source_audio_place_outputs(monitoring_type: c_int) -> u8;
    /// `size_t obs_source_audio_place_action_frame(uint64_t timestamp,
    /// uint64_t audio_ts, size_t sample_rate)`.
    pub fn oracle_obs_source_audio_place_action_frame(
        timestamp: u64,
        audio_ts: u64,
        sample_rate: usize,
    ) -> usize;
    /// `bool obs_source_audio_place_action_stops(size_t frame)`.
    pub fn oracle_obs_source_audio_place_action_stops(frame: usize) -> u8;
    /// `size_t obs_source_audio_place_fill_end(size_t frame_num, size_t
    /// new_frame_num)`.
    pub fn oracle_obs_source_audio_place_fill_end(frame_num: usize, new_frame_num: usize) -> usize;
    /// `bool obs_source_audio_place_mix_on(uint32_t audio_mixers, size_t
    /// mix)`.
    pub fn oracle_obs_source_audio_place_mix_on(audio_mixers: u32, mix: usize) -> u8;
    /// The header's values and sizes, by index into [`VALUE_NAMES`];
    /// `INT64_MIN` past the end.
    pub fn oracle_source_audio_place_value(which: usize) -> i64;
}
