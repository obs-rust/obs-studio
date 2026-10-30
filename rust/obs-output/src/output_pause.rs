//! Safe core of `libobs/obs-output-pause.c.h`: the pause plan of
//! `obs-output.c`.
//!
//! `obs_output_pause()` runs it each time a recording (or any output that
//! can pause) is paused or unpaused: every encoder of the output starts or
//! ends its pause at the same time on the video frame grid, or none does.
//! `obs-output.c` keeps `obs_output_valid()`, every atomic on `active`,
//! `paused` and the encoders' `paused` (each loaded where it always was),
//! every `pause.mutex` lock and unlock (in the slots' order, released in
//! reverse), the frame interval read and `os_gettime_ns()` in their order,
//! `get_first_video_encoder_index()`, the stores to each record, the
//! `pause` and `unpause` signals and the log. It passes the values here.
//! These keep no state, take no lock and call nothing.
//!
//! - `obs_output_pause()`: [`allowed`] (the output can pause), then, once
//!   the output is active, [`already`] (it is already in the wanted state,
//!   which succeeds at once);
//! - `obs_encoded_output_pause()`: [`lock_set`] (whose pause records are
//!   locked, checked and updated);
//! - `get_closest_v_ts()`: [`closest_ts`] (the pause time, on the video
//!   frame grid);
//! - `pause_can_start()`, `pause_can_stop()` and `end_pause()`: [`started`]
//!   and [`ended`], one test per field of the record, which the caller joins
//!   with `!` and `&&` in the order it always read them (the end is read
//!   only once the start test passed); [`can_start`] and [`can_stop`] are
//!   those joins, for safe Rust callers and the tests;
//! - `end_pause()`: [`offset`].
//!
//! The encoders come from the output's slots, which plugins and scripts
//! fill through `obs_output_set_video_encoder2()` and
//! `obs_output_set_audio_encoder()`, and the times from the clock and the
//! encoders; nothing here can panic on any input.
//!
//! Intentional differences:
//! - one encoder in several slots is locked, checked and updated once, at
//!   its first slot (video slots first). The C marks every slot with an
//!   encoder, so `obs_encoded_output_pause()` locks that encoder's plain
//!   `pause.mutex` a second time and the caller hangs; the setters check
//!   only the slot they set, so a plugin or script can put one encoder in
//!   two slots;
//! - a frame interval of 0 gives the clock as the pause time. The C
//!   divides by it, and `video_frame_interval_ns` stays 0 until video is
//!   first set up, so a raw output that can pause, in a libobs host that
//!   never reset video, divides by zero.

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

/// `OBS_OUTPUT_CAN_PAUSE`.
pub const OBS_OUTPUT_CAN_PAUSE: u32 = 1 << 5;
/// `MAX_OUTPUT_VIDEO_ENCODERS`.
pub const MAX_VIDEO_ENCODERS: usize = 10;
/// `MAX_OUTPUT_AUDIO_ENCODERS`.
pub const MAX_AUDIO_ENCODERS: usize = 6;

/// The slots whose encoder's pause record `obs_encoded_output_pause()`
/// locks, checks and updates: bit `i` of `video` for `video_encoders[i]`,
/// bit `i` of `audio` for `audio_encoders[i]`. The caller locks them video
/// slots first, each in slot order, and unlocks them in the reverse order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LockSet {
    /// The marked video slots.
    pub video: u32,
    /// The marked audio slots.
    pub audio: u32,
}

/// `obs_output_pause()`: an output with `flags` (`info.flags`) can pause.
#[must_use]
pub fn allowed(flags: u32) -> bool {
    flags & OBS_OUTPUT_CAN_PAUSE != 0
}

/// `obs_output_pause()`, for an active output: it is `paused` and the call
/// asks for `pause`, so it succeeds at once, with no signal and no log.
#[must_use]
pub fn already(paused: bool, pause: bool) -> bool {
    paused == pause
}

/// `obs_encoded_output_pause()`: the slots to lock, check and update, from
/// each slot's encoder as an opaque id (0 for an empty slot). Each distinct
/// encoder is marked once, at its first slot, video slots first.
#[must_use]
pub fn lock_set(
    video: &[usize; MAX_VIDEO_ENCODERS],
    audio: &[usize; MAX_AUDIO_ENCODERS],
) -> LockSet {
    let mut set = LockSet::default();
    let mut seen = [0_usize; MAX_VIDEO_ENCODERS + MAX_AUDIO_ENCODERS];
    let mut count = 0_usize;
    let mut mark = |id: usize, bit: u32| -> u32 {
        let earlier = seen.get(..count).unwrap_or_default();
        if id == 0 || earlier.contains(&id) {
            return 0;
        }
        if let Some(slot) = seen.get_mut(count) {
            *slot = id;
            count = count.saturating_add(1);
        }
        bit
    };
    for (bit, &id) in (0_u32..).zip(video) {
        set.video |= mark(id, 1_u32.wrapping_shl(bit));
    }
    for (bit, &id) in (0_u32..).zip(audio) {
        set.audio |= mark(id, 1_u32.wrapping_shl(bit));
    }
    set
}

/// `get_closest_v_ts()`: the pause time for a clock of `now`, in ns: the
/// video frame grid of `interval` ns (`video_frame_interval_ns`) through
/// the record's last video timestamp (`last_video_ts`), two frames ahead,
/// in wrapping 64-bit arithmetic as the C computes it. An `interval` of 0
/// gives `now`.
#[must_use]
pub fn closest_ts(last_video_ts: u64, interval: u64, now: u64) -> u64 {
    // `last + ((now - last + 2 * interval) / interval) * interval`, wrapping
    // as `uint64_t` does.
    let i2 = interval.wrapping_mul(2);
    match now
        .wrapping_sub(last_video_ts)
        .wrapping_add(i2)
        .checked_div(interval)
    {
        Some(frames) => last_video_ts.wrapping_add(frames.wrapping_mul(interval)),
        None => now,
    }
}

/// `pause_can_start()` and `pause_can_stop()`: a record whose start is
/// `ts_start` started a pause.
#[must_use]
pub fn started(ts_start: u64) -> bool {
    ts_start != 0
}

/// `pause_can_start()`, `pause_can_stop()` and `end_pause()`: a record whose
/// end is `ts_end` ended its pause (until the encoders clear it), so
/// `end_pause()` sets the end only once.
#[must_use]
pub fn ended(ts_end: u64) -> bool {
    ts_end != 0
}

/// `pause_can_start()`: a record (`ts_start`, `ts_end`) can start a pause:
/// it has neither ([`started`] and [`ended`] joined as the C joins them).
#[must_use]
pub fn can_start(ts_start: u64, ts_end: u64) -> bool {
    !started(ts_start) && !ended(ts_end)
}

/// `pause_can_stop()`: a record (`ts_start`, `ts_end`) can end its pause:
/// it started and has not ended.
#[must_use]
pub fn can_stop(ts_start: u64, ts_end: u64) -> bool {
    started(ts_start) && !ended(ts_end)
}

/// `end_pause()`: the record's offset once a pause from `ts_start` to
/// `ts_end` ended, from its offset before (`ts_offset`), in wrapping 64-bit
/// arithmetic as the C computes it.
#[must_use]
pub fn offset(ts_offset: u64, ts_start: u64, ts_end: u64) -> u64 {
    ts_offset.wrapping_add(ts_end.wrapping_sub(ts_start))
}
