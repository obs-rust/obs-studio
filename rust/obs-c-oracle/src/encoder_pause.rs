//! `oracle_*` build of `libobs/obs-encoder-pause.c.h`
//! (oracle/encoder_pause.c): the encoder pause checks, renamed.

use std::ffi::c_int;

unsafe extern "C" {
    pub fn oracle_obs_encoder_pause_off(ts_start: u64) -> bool;

    pub fn oracle_obs_encoder_pause_video_end(ts_end: u64, ts: u64) -> bool;

    pub fn oracle_obs_encoder_pause_video_drop(ts_start: u64, ts: u64) -> bool;

    pub fn oracle_obs_encoder_pause_audio_end(timestamp: u64, duration_ns: u64) -> u64;

    pub fn oracle_obs_encoder_pause_audio_start(
        ts_start: u64,
        timestamp: u64,
        end_ts: u64,
    ) -> c_int;

    pub fn oracle_obs_encoder_pause_audio_paused(ts_end: u64, timestamp: u64, end_ts: u64)
    -> c_int;

    pub fn oracle_obs_encoder_pause_audio_cut_ns(ts: u64, timestamp: u64) -> u64;

    pub fn oracle_obs_encoder_pause_audio_kept(cut_frames: u64) -> u32;

    pub fn oracle_obs_encoder_pause_audio_left(frames: u32, cut_frames: u64) -> u32;

    pub fn oracle_obs_encoder_pause_audio_drop(frames: u32) -> bool;
}
