//! `oracle_*` build of `libobs/obs-output-raw-audio.c.h`
//! (oracle/output_raw_audio.c): the raw output audio admission and the
//! capture hookup, renamed.
//!
//! Every function takes and returns plain values; nothing here divides.
//! `oracle_obs_output_raw_audio_mix_on` is the C's shift by `idx`: callers
//! keep `idx` below the width of `usize`, where the C is defined.

/// Index of each entry in `oracle_output_raw_audio_constant`, in the C's
/// order.
pub const CONSTANT_NAMES: [&str; 2] = [
    "OBS_OUTPUT_RAW_AUDIO_FLAG_VIDEO",
    "OBS_OUTPUT_RAW_AUDIO_FRAMES",
];

unsafe extern "C" {
    pub fn oracle_obs_output_raw_audio_passthrough(flags: u32) -> bool;

    pub fn oracle_obs_output_raw_audio_video_unset(video_start_ts: u64) -> bool;

    pub fn oracle_obs_output_raw_audio_before_video(timestamp: u64, video_start_ts: u64) -> bool;

    pub fn oracle_obs_output_raw_audio_ends_before(
        timestamp: u64,
        duration_ns: u64,
        video_start_ts: u64,
    ) -> bool;

    pub fn oracle_obs_output_raw_audio_cut_ns(video_start_ts: u64, timestamp: u64) -> u64;

    pub fn oracle_obs_output_raw_audio_cut_ts(timestamp: u64, cut_ns: u64) -> u64;

    pub fn oracle_obs_output_raw_audio_cut_bytes(audio_size: usize, cut_frames: u64) -> usize;

    pub fn oracle_obs_output_raw_audio_cut_frames(frames: u32, cut_frames: u64) -> u32;

    pub fn oracle_obs_output_raw_audio_start_unset(audio_start_ts: u64) -> bool;

    pub fn oracle_obs_output_raw_audio_block_bytes(audio_size: usize) -> usize;

    pub fn oracle_obs_output_raw_audio_push_bytes(frames: u32, audio_size: usize) -> usize;

    pub fn oracle_obs_output_raw_audio_can_pop(buffered: usize, block_bytes: usize) -> bool;

    pub fn oracle_obs_output_raw_audio_block_ts(audio_start_ts: u64, frames_ns: u64) -> u64;

    pub fn oracle_obs_output_raw_audio_paused_ts(timestamp: u64, ts_offset: u64) -> u64;

    pub fn oracle_obs_output_raw_audio_next_frames(total_audio_frames: usize) -> usize;

    /// The C shifts by `idx`: undefined from the width of `usize` up.
    pub fn oracle_obs_output_raw_audio_mix_on(mixer_mask: usize, idx: usize) -> bool;

    pub fn oracle_obs_output_raw_audio_interleave(has_video: bool, has_audio: bool) -> bool;

    pub fn oracle_obs_output_raw_audio_delay_on(delay_sec: u32) -> bool;

    pub fn oracle_obs_output_raw_audio_delay_ns(delay_sec: u32) -> u64;

    pub fn oracle_output_raw_audio_constant(which: usize) -> u64;
}
