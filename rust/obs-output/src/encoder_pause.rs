//! Safe core of `libobs/obs-encoder-pause.c.h`: the pause checks of
//! `obs-encoder.c`.
//!
//! An output that pauses gives each of its encoders a pause record: the
//! start (`ts_start`, 0 when no pause is on) and, once it unpauses, the end
//! (`ts_end`, 0 until then), both on the video frame grid. Each encoder's
//! video and audio thread then asks these, frame by frame and buffer by
//! buffer, what a paused recording drops and where each encoder's pause
//! ends. `obs-encoder.c` keeps every lock and unlock of `pause->mutex`, the
//! `last_video_ts` store, every read of the record and the buffer (each
//! where it was), both conversions that divide by the sample rate or by
//! 1000000000 (`audio_frames_to_ns()` and `ns_to_audio_frames()`, so the
//! zero-rate and MSVC x64 traps of `util_mul_div64()` stay where they are),
//! every write to the record and the buffer, the data pointer advance of
//! each plane, the `ts_offset` subtraction, `pause_reset()` and every
//! caller. It passes the values here. These keep no state, take no lock and
//! call nothing.
//!
//! - `video_pause_check()`: [`off`] (no pause, the frame passes), then
//!   [`video_end`] (the frame ends the pause) and, for a frame that does
//!   not, [`video_drop`];
//! - `audio_pause_check()`: [`off`], [`audio_end`] (the buffer's end),
//!   [`audio_start`] (where the pause start falls) and, for a buffer after
//!   the start, [`audio_paused`] (where the pause end falls); for each cut,
//!   [`audio_cut_ns`], then [`audio_kept`] (`pause_audio()`) or
//!   [`audio_left`] (`unpause_audio()`), and [`audio_drop`].
//!
//! Nothing here can panic on any input: the sums and differences wrap as
//! the C's `uint64_t` and `uint32_t` maths does.
//!
//! Intentional differences:
//! - a video pause ends on the first frame at or after a set `ts_end`
//!   ([`video_end`]). The C ends it only on a frame stamped exactly
//!   `ts_end`; the unpause time is on the frame grid of the output's first
//!   video encoder, so an encoder with a frame rate divisor of 2 or more,
//!   fed through video-io, may never get that frame, drops every later one,
//!   and keeps its record set, so the output refuses every later pause;
//! - a paused audio buffer that starts after a set `ts_end` ends the pause
//!   and passes as it is ([`AudioPaused::Resume`]). The C drops it, and
//!   every later buffer: when a pause and its unpause fall inside one buffer
//!   (a quick double press of the pause hotkey, or a script), the buffer is
//!   cut at the start and the end is never looked at, so the encoder gets
//!   no more audio and its record stays set. The audio from `ts_end` to the
//!   end of the buffer that was cut, at most one buffer, is still lost.

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

/// Where [`audio_start`] puts the pause start against a buffer
/// (`OBS_ENCODER_PAUSE_AUDIO_START_*`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioStart {
    /// The pause starts after the buffer, so it passes.
    After,
    /// The pause starts inside the buffer (or at either end), so the buffer
    /// is cut there (`pause_audio()`).
    Inside,
    /// The pause started before the buffer, so [`audio_paused`] decides.
    Before,
}

/// What [`audio_paused`] does with a buffer after the pause start
/// (`OBS_ENCODER_PAUSE_AUDIO_PAUSED_*`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioPaused {
    /// The buffer is dropped.
    Drop,
    /// The pause ends inside the buffer (or at either end), so the buffer
    /// is cut there and the pause ends (`unpause_audio()`).
    Unpause,
    /// The pause ended before the buffer: the pause ends (`ts_start` and
    /// `ts_end` are cleared) and the buffer passes as it is.
    Resume,
}

/// `video_pause_check()` and `audio_pause_check()`: no pause is on
/// (`ts_start` is 0), so the frame or buffer passes.
#[must_use]
pub fn off(ts_start: u64) -> bool {
    ts_start == 0
}

/// `video_pause_check()`, for a pause that is on: the frame stamped `ts`
/// ends the pause (`ts_start` and `ts_end` are cleared) and passes, from the
/// pause end (`ts_end`, 0 until the pause ends): a frame exactly at
/// `ts_end`, or the first one after a set `ts_end`.
#[must_use]
pub fn video_end(ts_end: u64, ts: u64) -> bool {
    ts == ts_end || (ts_end != 0 && ts > ts_end)
}

/// `video_pause_check()`, for a frame that does not end the pause: the
/// frame is dropped, at or after the pause start.
#[must_use]
pub fn video_drop(ts_start: u64, ts: u64) -> bool {
    ts >= ts_start
}

/// `audio_pause_check()`: the buffer's end, `timestamp + duration_ns`
/// (`audio_frames_to_ns(sample_rate, frames)`), wrapping as a `uint64_t`
/// sum.
#[must_use]
pub fn audio_end(timestamp: u64, duration_ns: u64) -> u64 {
    timestamp.wrapping_add(duration_ns)
}

/// `audio_pause_check()`, for a pause that is on: where its start falls
/// against the buffer from `timestamp` to `end_ts`.
#[must_use]
pub fn audio_start(ts_start: u64, timestamp: u64, end_ts: u64) -> AudioStart {
    if ts_start < timestamp {
        AudioStart::Before
    } else if ts_start <= end_ts {
        AudioStart::Inside
    } else {
        AudioStart::After
    }
}

/// `audio_pause_check()`, for a buffer after the pause start: what the
/// buffer from `timestamp` to `end_ts` does, from the pause end (`ts_end`,
/// 0 until the pause ends).
#[must_use]
pub fn audio_paused(ts_end: u64, timestamp: u64, end_ts: u64) -> AudioPaused {
    if ts_end >= timestamp && ts_end <= end_ts {
        AudioPaused::Unpause
    } else if ts_end != 0 && ts_end < timestamp {
        AudioPaused::Resume
    } else {
        AudioPaused::Drop
    }
}

/// `pause_audio()` and `unpause_audio()`: the nanoseconds from the buffer's
/// start to the cut at `ts` (`ts_start` or `ts_end`), `ts - timestamp`,
/// wrapping as a `uint64_t` difference.
#[must_use]
pub fn audio_cut_ns(ts: u64, timestamp: u64) -> u64 {
    ts.wrapping_sub(timestamp)
}

/// `pause_audio()`: the frames the buffer keeps before the cut,
/// `cut_frames` (`ns_to_audio_frames(sample_rate, cut_ns)`) truncated to
/// `uint32_t`.
#[must_use]
pub fn audio_kept(cut_frames: u64) -> u32 {
    truncate(cut_frames)
}

/// `unpause_audio()`: the frames the buffer keeps after the cut,
/// `frames - (uint32_t)cut_frames`, wrapping as a `uint32_t` difference.
#[must_use]
pub fn audio_left(frames: u32, cut_frames: u64) -> u32 {
    frames.wrapping_sub(truncate(cut_frames))
}

/// `audio_pause_check()`, after a cut: no frame is left, so the buffer is
/// dropped.
#[must_use]
pub fn audio_drop(frames: u32) -> bool {
    frames == 0
}

/// `(uint32_t)cut_frames`: the low 32 bits.
fn truncate(cut_frames: u64) -> u32 {
    u32::try_from(cut_frames & u64::from(u32::MAX)).unwrap_or(u32::MAX)
}
