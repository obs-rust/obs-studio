//! Safe core of `libobs/obs-encoder-audio-start.c.h`: the encoder audio start
//! alignment and the setter gates of `obs-encoder.c`.
//!
//! One function per decision, each asked where the C makes it.
//! `obs-encoder.c` keeps every read of the encoder, its group and its paired
//! encoders, in its order and under its conditions, the group mutex, the
//! weak references and releases, the deques, `do_encode()`, the pause checks,
//! the group reconfigure, the active and initialized checks, the
//! `fps_override` free and create, the logs and both `util_mul_div64()`
//! conversions between nanoseconds and frames. It passes their results here.
//! These keep no state and call nothing in libobs.
//!
//! - `receive_video()`: [`group_ready`] (the group's start against the
//!   frame), [`paired_late`] (asked for each paired audio encoder the walk
//!   reaches that has received audio; the walk stops at the first late one)
//!   and [`video_pts`] (the pts step after an encode).
//! - `buffer_audio()`: [`packet_size`], [`paired_start`] and
//!   [`unpaired_start`] (its two branches), [`no_video`] and, once the video
//!   has started, [`packet_start`]: wait, or start with the packet
//!   truncated, the buffer cleared, or from the buffer.
//! - `calc_offset_size()`: [`offset_ns`] and [`offset_size`], around the
//!   conversion to frames; `start_from_buffer()`: [`buffer_offset`];
//!   `push_back_audio()`: [`push_size`].
//! - `receive_audio()` and `send_audio_data()`: [`can_send`] (the send loop)
//!   and [`audio_pts`] (the pts step).
//! - `obs_encoder_set_gpu_scale_type()` and
//!   `obs_encoder_set_frame_rate_divisor()`: [`not_video`] (the first
//!   refusal, [`crate::encoder_size::scaled_gate`]'s) and [`divisor_ok`].
//!
//! Every sum, difference and product wraps as the C's unsigned arithmetic
//! does; the pts steps wrap in `i64`. No input comes from outside OBS
//! (timestamps from the audio and video threads, sizes and rates from libobs
//! and the encoder plugins, the type from a plugin, the divisor from the
//! frontend), and nothing here can panic on any input.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable
)]

use crate::encoder_size::ScaledGate;

/// `OBS_ENCODER_VIDEO` (`enum obs_encoder_type`, `obs-encoder.h`).
pub const OBS_ENCODER_VIDEO: i32 = crate::encoder_size::OBS_ENCODER_VIDEO;

/// What `buffer_audio()` does with a packet once the paired video encoder
/// has started (its start timestamp is not 0).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PacketStart {
    /// The packet ends at or before the video's start: it is buffered and
    /// the audio does not start.
    Wait,
    /// The audio starts at the video's start.
    Start {
        /// The packet starts before the video: its frames before the
        /// video's start are dropped (`calc_offset_size()`).
        truncate: bool,
        /// The packet starts at or before the video: the buffered audio is
        /// cleared.
        clear: bool,
        /// The video started before the packet: the buffered audio is cut
        /// at the video's start and kept (`start_from_buffer()`).
        from_buffer: bool,
    },
}

/// `receive_video()`, for an encoder in a group that has not started: the
/// group's start timestamp (read under its mutex) equals the frame's.
#[must_use]
pub fn group_ready(group_start: u64, timestamp: u64) -> bool {
    group_start == timestamp
}

/// `receive_video()`, for each paired audio encoder the walk reaches that
/// has received audio: its first raw timestamp is after the frame's, so the
/// video waits.
#[must_use]
pub fn paired_late(first_raw_ts: u64, timestamp: u64) -> bool {
    first_raw_ts > timestamp
}

/// `receive_video()` after a successful encode: `cur_pts` plus
/// `timebase_num * frame_rate_divisor`, a `u32` product as in the C.
#[must_use]
pub fn video_pts(cur_pts: i64, timebase_num: u32, frame_rate_divisor: u32) -> i64 {
    cur_pts.wrapping_add(i64::from(timebase_num.wrapping_mul(frame_rate_divisor)))
}

/// `buffer_audio()`: the bytes of one plane of a packet, `frames *
/// blocksize` (a `size_t` product).
#[must_use]
pub fn packet_size(frames: u32, blocksize: usize) -> usize {
    to_size(u64::from(frames)).wrapping_mul(blocksize)
}

/// `buffer_audio()`: the audio has not started (`start_ts` 0) and has a
/// paired video encoder.
#[must_use]
pub fn paired_start(start_ts: u64, has_paired: bool) -> bool {
    start_ts == 0 && has_paired
}

/// `buffer_audio()`, when [`paired_start`] was false: the audio has not
/// started and has no paired video encoder, so it starts at this packet.
#[must_use]
pub fn unpaired_start(start_ts: u64, has_paired: bool) -> bool {
    start_ts == 0 && !has_paired
}

/// `buffer_audio()`: the paired video encoder's start timestamp is 0 (no
/// video yet), so the audio does not start.
#[must_use]
pub fn no_video(v_start_ts: u64) -> bool {
    // No video yet, so don't start audio.
    v_start_ts == 0
}

/// `buffer_audio()` once the video has started: `timestamp` is the packet's,
/// `duration_ns` its length (`util_mul_div64(frames, 1000000000,
/// samplerate)`, computed by the C), `v_start_ts` the video's start. The
/// packet ends at `timestamp + duration_ns`, a wrapping `u64` sum.
#[must_use]
pub fn packet_start(timestamp: u64, duration_ns: u64, v_start_ts: u64) -> PacketStart {
    // The audio's starting point is still not synced with the video's, so
    // don't start it.
    let end_ts = timestamp.wrapping_add(duration_ns);
    if end_ts <= v_start_ts {
        return PacketStart::Wait;
    }
    // Ready to start the audio, truncated if necessary; with the video
    // before the packet, from the buffered audio instead.
    PacketStart::Start {
        truncate: timestamp < v_start_ts,
        clear: timestamp <= v_start_ts,
        from_buffer: v_start_ts < timestamp,
    }
}

/// `calc_offset_size()`: the nanoseconds from the audio's start to the
/// video's, `v_start_ts - a_start_ts` (wrapping).
#[must_use]
pub fn offset_ns(v_start_ts: u64, a_start_ts: u64) -> u64 {
    v_start_ts.wrapping_sub(a_start_ts)
}

/// `calc_offset_size()`: `offset_frames` (the offset converted with
/// `util_mul_div64(offset, samplerate, 1000000000)`, computed by the C) cast
/// to `size_t`, times `blocksize` (wrapping).
#[must_use]
pub fn offset_size(offset_frames: u64, blocksize: usize) -> usize {
    to_size(offset_frames).wrapping_mul(blocksize)
}

/// `start_from_buffer()`: the buffered audio's first raw timestamp is before
/// the video's start, so its front is dropped.
#[must_use]
pub fn buffer_offset(first_raw_ts: u64, v_start_ts: u64) -> bool {
    first_raw_ts < v_start_ts
}

/// `push_back_audio()`: the bytes of `size` left after dropping
/// `offset_size` from the front, or `None` (nothing pushed) when the offset
/// covers it.
#[must_use]
pub fn push_size(size: usize, offset_size: usize) -> Option<usize> {
    size.checked_sub(offset_size).filter(|&left| left != 0)
}

/// `receive_audio()`'s send loop: the buffered bytes of plane 0 hold a whole
/// frame.
#[must_use]
pub fn can_send(buffered: usize, framesize_bytes: usize) -> bool {
    buffered >= framesize_bytes
}

/// `send_audio_data()` after a successful encode: `cur_pts` plus
/// `framesize` (wrapping, as the C's `size_t` sum converted back).
#[must_use]
pub fn audio_pts(cur_pts: i64, framesize: usize) -> i64 {
    cur_pts.wrapping_add_unsigned(to_u64(framesize))
}

/// `obs_encoder_set_gpu_scale_type()` and
/// `obs_encoder_set_frame_rate_divisor()` after `obs_encoder_valid()`:
/// `encoder_type` (`info.type`) is not [`OBS_ENCODER_VIDEO`], the first
/// refusal. The active and initialized refusals follow in the C.
#[must_use]
pub fn not_video(encoder_type: i32) -> bool {
    crate::encoder_size::scaled_gate(encoder_type, false, false) == ScaledGate::NotVideo
}

/// `obs_encoder_set_frame_rate_divisor()` after the refusals: the divisor is
/// not 0.
#[must_use]
pub fn divisor_ok(frame_rate_divisor: u32) -> bool {
    frame_rate_divisor != 0
}

/// `(size_t)value`: the low bits of a `u64` that fit a `usize` (all of it
/// where `usize` is 64 bits).
fn to_size(value: u64) -> usize {
    let mask = u64::try_from(usize::MAX).unwrap_or(u64::MAX);
    usize::try_from(value & mask).unwrap_or(usize::MAX)
}

/// A `usize` as a `u64` (lossless on every target libobs builds for).
fn to_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}
