//! `oracle_*` build of `libobs/obs-encoder-audio-start.c.h`
//! (oracle/encoder_audio_start.c): the encoder audio start alignment and the
//! setter gates, renamed.
//!
//! The struct returned by value comes back as [`Plan`], whose layout
//! [`oracle_encoder_audio_start_layout`] checks. Nothing here divides.
//! `oracle_obs_encoder_audio_start_video_pts` is the C's `int64_t` sum:
//! callers keep `cur_pts + timebase_num * frame_rate_divisor` within
//! `i64`, where the C is defined.

use core::ffi::c_int;

/// Index of each entry in `oracle_encoder_audio_start_constant`, in the C's
/// order.
pub const CONSTANT_NAMES: [&str; 1] = ["OBS_ENCODER_AUDIO_START_TYPE_VIDEO"];

/// Index of each entry in `oracle_encoder_audio_start_layout`, in the C's
/// order.
pub const LAYOUT_NAMES: [&str; 6] = [
    "sizeof(struct obs_encoder_audio_start_plan)",
    "alignof(struct obs_encoder_audio_start_plan)",
    "offsetof(struct obs_encoder_audio_start_plan, wait)",
    "offsetof(struct obs_encoder_audio_start_plan, truncate)",
    "offsetof(struct obs_encoder_audio_start_plan, clear)",
    "offsetof(struct obs_encoder_audio_start_plan, from_buffer)",
];

/// `struct obs_encoder_audio_start_plan`, as the C returns it.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Plan {
    pub wait: bool,
    pub truncate: bool,
    pub clear: bool,
    pub from_buffer: bool,
}

unsafe extern "C" {
    pub fn oracle_obs_encoder_audio_start_group_ready(group_start: u64, timestamp: u64) -> bool;

    pub fn oracle_obs_encoder_audio_start_paired_late(first_raw_ts: u64, timestamp: u64) -> bool;

    /// The C's `int64_t` sum: undefined past `i64`'s range.
    pub fn oracle_obs_encoder_audio_start_video_pts(
        cur_pts: i64,
        timebase_num: u32,
        frame_rate_divisor: u32,
    ) -> i64;

    pub fn oracle_obs_encoder_audio_start_size(frames: u32, blocksize: usize) -> usize;

    pub fn oracle_obs_encoder_audio_start_paired(start_ts: u64, has_paired: bool) -> bool;

    pub fn oracle_obs_encoder_audio_start_unpaired(start_ts: u64, has_paired: bool) -> bool;

    pub fn oracle_obs_encoder_audio_start_no_video(v_start_ts: u64) -> bool;

    pub fn oracle_obs_encoder_audio_start_packet(
        timestamp: u64,
        duration_ns: u64,
        v_start_ts: u64,
    ) -> Plan;

    pub fn oracle_obs_encoder_audio_start_offset_ns(v_start_ts: u64, a_start_ts: u64) -> u64;

    pub fn oracle_obs_encoder_audio_start_offset_size(
        offset_frames: u64,
        blocksize: usize,
    ) -> usize;

    pub fn oracle_obs_encoder_audio_start_buffer_offset(first_raw_ts: u64, v_start_ts: u64)
    -> bool;

    pub fn oracle_obs_encoder_audio_start_push_size(size: usize, offset_size: usize) -> usize;

    pub fn oracle_obs_encoder_audio_start_can_send(buffered: usize, framesize_bytes: usize)
    -> bool;

    pub fn oracle_obs_encoder_audio_start_audio_pts(cur_pts: i64, framesize: usize) -> i64;

    pub fn oracle_obs_encoder_audio_start_not_video(encoder_type: c_int) -> bool;

    pub fn oracle_obs_encoder_audio_start_divisor_ok(frame_rate_divisor: u32) -> bool;

    /// The entry named `CONSTANT_NAMES[which]`; -1 out of range.
    pub fn oracle_encoder_audio_start_constant(which: usize) -> i64;

    /// The entry named `LAYOUT_NAMES[which]`; `usize::MAX` out of range.
    pub fn oracle_encoder_audio_start_layout(which: usize) -> usize;
}
