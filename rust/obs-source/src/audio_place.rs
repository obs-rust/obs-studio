//! Safe core of `libobs/obs-source-audio-place.c.h`: the source audio
//! placement and action timeline of `obs-source.c`.
//!
//! - [`started`] and [`older`]: the reset of `source_output_audio_place()`
//!   (no `audio_ts`, or an older packet), the push-back rule of
//!   `source_output_audio_data()` and the action clamp of
//!   `apply_audio_actions()`;
//! - [`size`], [`offset`], [`fits`] and [`pop_back`]: the packet's bytes,
//!   its byte placement (`get_buf_placement()`), the drop past
//!   [`MAX_BUF_SIZE`] and how much is popped off the back of each channel;
//! - [`outputs`]: nothing for a monitor-only source;
//! - [`action_frame`], [`action_stops`], [`fill_end`] and [`mix_on`]: each
//!   action's frame, the stop at [`FRAMES`], how far the previous volume
//!   fills and the mixes multiplied.
//!
//! Each decision is its own function, asked at its own point, so
//! `obs-source.c` keeps every read where it was: `audio_ts` (again after
//! `reset_audio_data()`, and only where the C's `&&` and `||` reach it),
//! `monitoring_type`, the channels' sizes and `audio_mixers` once per mix.
//! It keeps `audio_buf_mutex` and `audio_actions_mutex`, the sample rate
//! and channel count, `reset_audio_data()`, the deques, `da_erase()`,
//! `apply_audio_action()`, every `get_source_volume()` call (before the
//! loop and after each applied action), the volume array and
//! `multiply_vol_data()`. Nothing here keeps state, takes a lock or calls
//! libobs.
//!
//! The arithmetic wraps where the C's unsigned arithmetic does: the
//! `uint64_t` differences, the `size_t` sums, products and difference.
//! The frame conversions are `util_mul_div64(diff, rate, 1e9)` with the
//! host's formula ([`crate::transition_render`]'s copy): the wrapping
//! `uint64_t` formula, or on MSVC x64 the exact 128-bit quotient, which in
//! the C faults when it does not fit in 64 bits and here saturates (an
//! intentional difference, only for a rate above 1e9 Hz). [`mix_on`] past
//! the `u32`'s width says no mix, where the C's `int` shift is undefined
//! (from 31 on; libobs asks only 0 to 5).
//!
//! Characterized, not endorsed (`test_source_audio_place_steps`), and kept:
//! a packet or action less than a sample after `audio_ts` lands on frame 0;
//! a packet larger than [`MAX_BUF_SIZE`] is dropped whole; a push back
//! checks only the first channel; the action loop stops at the first action
//! at or past [`FRAMES`], so later due ones wait; a monitoring type outside
//! the enum outputs.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::as_conversions,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable
)]

use crate::transition_render::{mul_div64, to_size};

/// Nanoseconds per second, the divisor of the frame conversions.
const NS_PER_SEC: u64 = 1_000_000_000;
/// `sizeof(float)`.
const SAMPLE_BYTES: usize = size_of::<f32>();

/// `AUDIO_OUTPUT_FRAMES` (`media-io/audio-io.h`).
pub const FRAMES: usize = 1024;
/// `MAX_BUF_SIZE` (`obs-source.c`): `1000 * AUDIO_OUTPUT_FRAMES *
/// sizeof(float)` bytes per channel.
pub const MAX_BUF_SIZE: usize = 1000 * FRAMES * SAMPLE_BYTES;
/// `OBS_MONITORING_TYPE_MONITOR_ONLY` (`enum obs_monitoring_type`, `obs.h`).
pub const MONITOR_ONLY: i32 = 1;

/// Whether the source has an audio timestamp: `audio_ts != 0`.
#[must_use]
pub fn started(audio_ts: u64) -> bool {
    audio_ts != 0
}

/// Whether `timestamp` is older than `audio_ts`.
#[must_use]
pub fn older(timestamp: u64, audio_ts: u64) -> bool {
    timestamp < audio_ts
}

/// A packet's bytes per channel: `frames * sizeof(float)` in `size_t`.
#[must_use]
pub fn size(frames: u32) -> usize {
    usize::try_from(frames)
        .unwrap_or(usize::MAX)
        .wrapping_mul(SAMPLE_BYTES)
}

/// `get_buf_placement()` in bytes: `(size_t)util_mul_div64(timestamp -
/// audio_ts, sample_rate, 1e9) * sizeof(float)`.
#[must_use]
pub fn offset(timestamp: u64, audio_ts: u64, sample_rate: u32) -> usize {
    frames(timestamp, audio_ts, u64::from(sample_rate)).wrapping_mul(SAMPLE_BYTES)
}

/// Whether `size` bytes from `start` stay within [`MAX_BUF_SIZE`].
#[must_use]
pub fn fits(start: usize, size: usize) -> bool {
    start.wrapping_add(size) <= MAX_BUF_SIZE
}

/// The bytes popped off the back of a channel after the place:
/// `buf_size - (placement + size)`.
#[must_use]
pub fn pop_back(buf_size: usize, placement: usize, size: usize) -> usize {
    buf_size.wrapping_sub(placement.wrapping_add(size))
}

/// Whether the packet goes to the buffers: not [`MONITOR_ONLY`].
#[must_use]
pub fn outputs(monitoring_type: i32) -> bool {
    monitoring_type != MONITOR_ONLY
}

/// An action's frame: `conv_time_to_frames(sample_rate, timestamp -
/// audio_ts)`.
#[must_use]
pub fn action_frame(timestamp: u64, audio_ts: u64, sample_rate: usize) -> usize {
    frames(
        timestamp,
        audio_ts,
        u64::try_from(sample_rate).unwrap_or(u64::MAX),
    )
}

/// Whether the action loop stops at this frame: `frame >= FRAMES`.
#[must_use]
pub fn action_stops(frame: usize) -> bool {
    frame >= FRAMES
}

/// How far the previous volume fills before an action at `new_frame_num`.
#[must_use]
pub fn fill_end(frame_num: usize, new_frame_num: usize) -> usize {
    if new_frame_num > frame_num {
        new_frame_num
    } else {
        frame_num
    }
}

/// Whether the volume array multiplies mix `mix`: bit `mix` of
/// `audio_mixers`.
#[must_use]
pub fn mix_on(audio_mixers: u32, mix: usize) -> bool {
    u32::try_from(mix)
        .ok()
        .and_then(|shift| 1_u32.checked_shl(shift))
        .is_some_and(|bit| audio_mixers & bit != 0)
}

/// `(size_t)util_mul_div64(timestamp - audio_ts, sample_rate, 1e9)`: the
/// frames from `audio_ts` to `timestamp`, the difference wrapping.
fn frames(timestamp: u64, audio_ts: u64, sample_rate: u64) -> usize {
    to_size(mul_div64(
        timestamp.wrapping_sub(audio_ts),
        sample_rate,
        NS_PER_SEC,
    ))
}
