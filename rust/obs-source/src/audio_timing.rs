//! Safe core of `libobs/obs-source-audio-timing.c.h`: where
//! `source_output_audio_data()` places each audio packet of a source.
//!
//! A source hands libobs audio packets stamped with its own clock: the
//! system clock (audio capture), or a media file's timeline. libobs places
//! each packet in system time, in the source's buffer, by its timestamp plus
//! the source's `timing_adjust`, its sync offset and its resampler offset,
//! and appends a packet that continues the previous one instead of placing
//! it, so that timestamp jitter does not open gaps.
//!
//! This is only the logic, on plain values, in two steps around the
//! source's `audio_buf_mutex`, which `obs-source.c.h` holds between them:
//! - [`pre_lock`]: the direct timestamp check (within [`MAX_TS_VAR`] of the
//!   system clock: no `timing_adjust`), then the [`Action`] against the
//!   expected timestamp (`next_audio_ts_min`): reset the timing, a timestamp
//!   jump, smoothing onto the expected timestamp (with the drift error for
//!   the resampler and the re-anchor of an unbuffered, decoupled async
//!   source) or a log line; and the next expected timestamp;
//! - [`under_lock`]: append or place, the reset of the timing when the
//!   packet's system time jumped and the audio was not continuous (#522),
//!   the sync and resampler offsets, `next_audio_sys_ts_min`, and the rule
//!   that a changed sync offset places the packet (#333).
//!
//! [`buffer_start`] is where `reset_audio_data_at_sys_time()` starts an
//! emptied buffer (#286).
//!
//! `obs-source.c.h` keeps everything else: it reads the source's fields,
//! calls a step and stores what it returns, in the order the C always
//! stored it (`timing_set` and `timing_adjust` are volatile: the async
//! video thread reads and writes them); it runs `handle_ts_jump()` (the
//! lock and the buffer clear), the resampler's compensation, the deque, the
//! signal, the muting and the log lines; and it computes the packet's
//! duration, so nothing here divides. Nothing here keeps state, takes a
//! lock or calls libobs.
//!
//! Every sum and difference wraps, as `uint64_t` does in the C, and the
//! drift error and the sync offset are the C's two's complement
//! conversions. Characterized, not endorsed (`test_source_audio_timing_steps`),
//! and kept: a negative sync offset larger than the packet's system time,
//! or a resampler offset larger than it, wraps to near `u64::MAX`; so does
//! `timestamp + timing_adjust`, which comes back to the system time only
//! because every later sum wraps too; the distance between two timestamps
//! is not modular, so timestamps on either side of `u64::MAX` are far
//! apart.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::as_conversions,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::unreachable
)]

/// `TS_SMOOTHING_THRESHOLD` (`obs-source.c.h`): a packet less than this
/// from where it is expected continues the previous one. In ns.
pub const TS_SMOOTHING_THRESHOLD: u64 = 70_000_000;

/// `MAX_TS_VAR` (`obs-internal.h`): a timestamp less than this from the
/// system clock is on it, and one more than this from where it is expected
/// jumped. In ns.
pub const MAX_TS_VAR: u64 = 2_000_000_000;

/// What `source_output_audio_data()` does with the packet's timestamp
/// before the lock (`enum obs_source_audio_ts_action`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    /// The timing is set but nothing is expected yet (`next_audio_ts_min`
    /// 0): keep the packet's timestamp.
    Keep,
    /// The timing is not set: `reset_audio_timing()` to this packet.
    Reset,
    /// More than [`MAX_TS_VAR`] from where it is expected, and not direct:
    /// `handle_ts_jump()`, which resets the timing to this packet and
    /// empties the buffer.
    Jump,
    /// Less than [`TS_SMOOTHING_THRESHOLD`] from where it is expected: the
    /// packet continues the previous one and goes where it was expected.
    Smooth,
    /// Neither: log that it exceeded `TS_SMOOTHING_THRESHOLD` and keep the
    /// packet's timestamp.
    Exceeded,
}

/// What [`pre_lock`] reads: the packet and the source's fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PreLockInput {
    /// The packet's timestamp.
    pub timestamp: u64,
    /// `os_gettime_ns()` for this packet.
    pub os_time: u64,
    /// The packet's duration (`conv_frames_to_time()`).
    pub duration: u64,
    /// `source->next_audio_ts_min`: where the packet is expected, 0 for
    /// nowhere.
    pub next_audio_ts_min: u64,
    /// `source->timing_set`.
    pub timing_set: bool,
    /// `source->async_unbuffered`.
    pub async_unbuffered: bool,
    /// `source->async_decoupled`.
    pub async_decoupled: bool,
}

/// What [`pre_lock`] decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreLock {
    /// What to do with the packet's timestamp.
    pub action: Action,
    /// The timestamp is on the system clock: store `timing_adjust` 0, then
    /// `timing_set`, before the action.
    pub direct: bool,
    /// [`Action::Smooth`] of an unbuffered, decoupled async source: store
    /// [`PreLock::timing_adjust`].
    pub reanchor: bool,
    /// [`Action::Smooth`]: the packet continues the previous one.
    pub continuous: bool,
    /// The packet's timestamp after smoothing, before `timing_adjust`.
    pub timestamp: u64,
    /// The new `next_audio_ts_min`: the packet's end.
    pub next_audio_ts_min: u64,
    /// The `timing_adjust` that [`Action::Reset`], [`Action::Jump`] and
    /// the re-anchor store (`os_time - timestamp`); 0 otherwise.
    pub timing_adjust: u64,
    /// [`Action::Jump`] and [`Action::Exceeded`]: how far the timestamp is
    /// from where it was expected; 0 otherwise.
    pub diff: u64,
    /// [`Action::Smooth`]: how far the samples run ahead of the timestamps
    /// (expected minus actual), for the resampler's compensation; 0
    /// otherwise.
    pub drift_error: i64,
}

/// What [`under_lock`] reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UnderLockInput {
    /// [`PreLock::timestamp`].
    pub timestamp: u64,
    /// `source->timing_adjust` as read before the lock, for the packet.
    pub packet_timing_adjust: u64,
    /// The packet's own timestamp, before smoothing.
    pub data_timestamp: u64,
    /// As for [`pre_lock`].
    pub os_time: u64,
    /// `source->next_audio_ts_min`, as stored after [`pre_lock`].
    pub next_audio_ts_min: u64,
    /// `source->resample_offset`.
    pub resample_offset: u64,
    /// `source->sync_offset`.
    pub sync_offset: i64,
    /// `source->timing_adjust` as read under the lock.
    pub timing_adjust: u64,
    /// `source->next_audio_sys_ts_min`: where the buffer's data ends in
    /// system time, 0 for nowhere.
    pub next_audio_sys_ts_min: u64,
    /// `source->last_sync_offset`.
    pub last_sync_offset: i64,
    /// [`PreLock::continuous`].
    pub continuous: bool,
}

/// What [`under_lock`] decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnderLock {
    /// The system time the packet is placed at: with `timing_adjust`, the
    /// sync offset and the resampler offset.
    pub timestamp: u64,
    /// Append the packet to the buffer (when the buffer has a start)
    /// instead of placing it by [`UnderLock::timestamp`].
    pub push_back: bool,
    /// `reset_audio_timing()` to the packet's own timestamp: store
    /// `timing_set`, then [`UnderLock::timing_adjust`].
    pub reset_timing: bool,
    /// The `timing_adjust` after the step: the input's, or the reset's.
    pub timing_adjust: u64,
    /// The new `next_audio_sys_ts_min`.
    pub next_audio_sys_ts_min: u64,
    /// The new `last_sync_offset`.
    pub last_sync_offset: i64,
}

/// `uint64_diff()`: the distance between two timestamps, not modular.
pub const fn distance(ts1: u64, ts2: u64) -> u64 {
    ts1.abs_diff(ts2)
}

/// `obs_source_audio_timing_pre_lock()`.
pub fn pre_lock(input: PreLockInput) -> PreLock {
    let mut out = PreLock {
        action: Action::Keep,
        direct: false,
        reanchor: false,
        continuous: false,
        timestamp: input.timestamp,
        next_audio_ts_min: 0,
        timing_adjust: 0,
        diff: 0,
        drift_error: 0,
    };

    // detects 'directly' set timestamps as long as they're within a
    // certain threshold: timing_adjust = 0, timing_set = true
    out.direct = distance(input.timestamp, input.os_time) < MAX_TS_VAR;
    let timing_set = input.timing_set || out.direct;
    // reset_audio_timing(): the timing that puts this packet at os_time
    let reset_adjust = input.os_time.wrapping_sub(input.timestamp);

    if !timing_set {
        out.action = Action::Reset;
        out.timing_adjust = reset_adjust;
    } else if input.next_audio_ts_min != 0 {
        let diff = distance(input.next_audio_ts_min, input.timestamp);

        if diff > MAX_TS_VAR && !out.direct {
            // handle_ts_jump()
            out.action = Action::Jump;
            out.diff = diff;
            out.timing_adjust = reset_adjust;
        } else if diff < TS_SMOOTHING_THRESHOLD {
            out.action = Action::Smooth;
            if input.async_unbuffered && input.async_decoupled {
                out.reanchor = true;
                out.timing_adjust = reset_adjust;
            }
            // how far the samples run ahead of the timestamps: the C's
            // (int64_t) of the uint64_t difference
            out.drift_error = input
                .next_audio_ts_min
                .wrapping_sub(input.timestamp)
                .cast_signed();
            out.timestamp = input.next_audio_ts_min;
            out.continuous = true;
        } else {
            out.action = Action::Exceeded;
            out.diff = diff;
        }
    }

    out.next_audio_ts_min = out.timestamp.wrapping_add(input.duration);
    out
}

/// `obs_source_audio_timing_under_lock()`.
pub fn under_lock(input: UnderLockInput) -> UnderLock {
    let mut timestamp = input.timestamp.wrapping_add(input.packet_timing_adjust);
    let mut push_back = false;
    let mut reset_timing = false;
    let mut timing_adjust = input.timing_adjust;

    if input.next_audio_sys_ts_min == timestamp {
        push_back = true;
    } else if input.next_audio_sys_ts_min != 0 {
        let diff = distance(input.next_audio_sys_ts_min, timestamp);

        if diff < TS_SMOOTHING_THRESHOLD {
            push_back = true;
        } else if diff > MAX_TS_VAR && !input.continuous {
            // The audio jumped ahead of the video: resync to the system
            // clock. If the audio was continuous, only the timing of the
            // source's video moved (a Video Delay (Async) filter), and the
            // audio follows it (#522).
            reset_timing = true;
            timing_adjust = input.os_time.wrapping_sub(input.data_timestamp);
            timestamp = input.data_timestamp.wrapping_add(timing_adjust);
        }
    }

    // the C's uint64_t += int64_t converts the offset to uint64_t
    timestamp = timestamp
        .wrapping_add(input.sync_offset.cast_unsigned())
        .wrapping_sub(input.resample_offset);

    let next_audio_sys_ts_min = input.next_audio_ts_min.wrapping_add(timing_adjust);

    // A packet appended to the buffer ignores its timestamp, and so the
    // sync offset in it: after the offset changes, place the next packet by
    // its timestamp (#333).
    let mut last_sync_offset = input.last_sync_offset;
    if last_sync_offset != input.sync_offset {
        push_back = false;
        last_sync_offset = input.sync_offset;
    }

    UnderLock {
        timestamp,
        push_back,
        reset_timing,
        timing_adjust,
        next_audio_sys_ts_min,
        last_sync_offset,
    }
}

/// `obs_source_audio_timing_buffer_start()`: where
/// `reset_audio_data_at_sys_time()` starts a buffer emptied for packets at
/// `os_time`: `os_time` plus the sync offset minus the resampler offset, as
/// a packet placed on its own lands, or 0 for 0.
pub fn buffer_start(os_time: u64, sync_offset: i64, resample_offset: u64) -> u64 {
    if os_time == 0 {
        return os_time;
    }
    os_time
        .wrapping_add(sync_offset.cast_unsigned())
        .wrapping_sub(resample_offset)
}
