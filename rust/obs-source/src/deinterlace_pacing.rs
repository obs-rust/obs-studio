//! Safe core of `libobs/obs-source-deinterlace-pacing.c.h`: the frame
//! pacing of double-rate deinterlacing (Yadif 2x, Linear 2x, Discard 2x).
//!
//! An async source with a 2x mode shows each interlaced frame as two
//! fields: the first from the canvas tick that picks the frame, the second
//! once the canvas clock reaches the frame's timestamp plus
//! `deinterlace_offset` plus `deinterlace_half_duration`, less
//! [`TWOX_TOLERANCE`]. The offset maps the frame's clock onto the canvas
//! clock. These functions decide those values, and when an unbuffered
//! source moves on to its next frame:
//! - [`hold`]: an unbuffered source keeps its frame while the frame's end
//!   is ahead of the clock, unless it is [`TS_JUMP_THRESHOLD`] or more
//!   ahead (the clock jumped);
//! - [`wait_for_next`] and [`waiting_frame_ts`]: the "don't skip ahead"
//!   guard of `ready_deinterlace_frames()`, for an unbuffered source with
//!   two frames queued;
//! - [`half_duration`]: half the time from the frame before, truncated to
//!   32 bits;
//! - [`buffered_offset`]: a buffered source's offset, from the source clock
//!   that picks its frames (#395);
//! - [`reanchor`]: an unbuffered source's offset, kept unless the frame
//!   comes early or misses its tick and its first field (#395).
//!
//! `obs-source-deinterlace.c.h` keeps everything else: the `async_frames`
//! queue, the buffered walk, `first_frame()`, the textures and the render.
//! It reads the source's fields and `obs->video`, passes them in, and
//! stores what comes back, in the order the C always did. Nothing here
//! keeps state, takes a lock or calls libobs.
//!
//! Every sum and difference wraps, as `uint64_t` does in the C, and the
//! re-anchor's lateness is the C's `(int64_t)` conversion of the wrapped
//! difference. Characterized, not endorsed (`test_deinterlace_pacing_steps`),
//! and kept: half a duration of 2^32 ns or more is truncated; a timestamp
//! going backwards wraps the half duration, the guard's end and the waiting
//! timestamp; the first frame with no second one queued takes its half
//! duration against a `deinterlace_frame_ts` of 0 (half its own timestamp,
//! truncated); a frame clock ahead of the canvas wraps the offsets; and a
//! re-anchor's lateness of 2^63 or more counts as early.

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

/// `FRAME_TOLERANCE` (`obs-source-deinterlace.c.h`): a frame is due once the
/// source clock is this far past its timestamp. In ns.
pub const FRAME_TOLERANCE: u64 = 2_000_000;

/// `TWOX_TOLERANCE` (`obs-source-deinterlace.c.h`): the second field is
/// shown this much early. In ns.
pub const TWOX_TOLERANCE: u64 = 1_000_000;

/// `TS_JUMP_THRESHOLD` (`obs-source-deinterlace.c.h`): a frame's end this
/// far ahead of the clock or more means the clock jumped. In ns.
pub const TS_JUMP_THRESHOLD: u64 = 70_000_000;

/// `obs_source_deinterlace_pacing_hold()`: whether an unbuffered source
/// whose offset is set keeps the frame at `frame_ts`: `sys_time` is before
/// the frame's end, `frame_ts + offset + 2 * half_duration -`
/// [`TWOX_TOLERANCE`], by less than [`TS_JUMP_THRESHOLD`].
pub fn hold(frame_ts: u64, offset: u64, half_duration: u32, sys_time: u64) -> bool {
    // Want to keep frame if it has not elapsed. The half duration is
    // widened before it is doubled, so it cannot wrap.
    let frame_end = frame_ts
        .wrapping_add(offset)
        .wrapping_add(u64::from(half_duration).wrapping_mul(2))
        .wrapping_sub(TWOX_TOLERANCE);
    // Process new frames if we think time jumped.
    sys_time < frame_end && frame_end.wrapping_sub(sys_time) < TS_JUMP_THRESHOLD
}

/// `obs_source_deinterlace_pacing_wait_for_next()`: whether an unbuffered
/// source whose offset is set waits with its two queued frames, at
/// `timestamp` and `next_timestamp`: `sys_time` is before `timestamp +
/// offset +` the frame's duration.
pub fn wait_for_next(timestamp: u64, next_timestamp: u64, offset: u64, sys_time: u64) -> bool {
    let duration = next_timestamp.wrapping_sub(timestamp);
    let frame_end = timestamp.wrapping_add(offset).wrapping_add(duration);
    // Don't skip ahead prematurely.
    sys_time < frame_end
}

/// `obs_source_deinterlace_pacing_waiting_frame_ts()`: the
/// `deinterlace_frame_ts` stored when [`wait_for_next`] waits: `timestamp`
/// less the frame's duration, `next_timestamp - timestamp`.
pub fn waiting_frame_ts(timestamp: u64, next_timestamp: u64) -> u64 {
    let duration = next_timestamp.wrapping_sub(timestamp);
    timestamp.wrapping_sub(duration)
}

/// `obs_source_deinterlace_pacing_half_duration()`: the
/// `deinterlace_half_duration` of a frame at `timestamp` after one at
/// `prev_ts`: half the difference, truncated to 32 bits.
pub fn half_duration(timestamp: u64, prev_ts: u64) -> u32 {
    let half = timestamp.wrapping_sub(prev_ts).wrapping_div(2);
    // (uint32_t): the low 32 bits
    let [b0, b1, b2, b3, ..] = half.to_le_bytes();
    u32::from_le_bytes([b0, b1, b2, b3])
}

/// `obs_source_deinterlace_pacing_buffered_offset()`: a buffered source's
/// `deinterlace_offset`, `video_time - last_frame_ts +`
/// [`FRAME_TOLERANCE`] `+` [`TWOX_TOLERANCE`].
pub fn buffered_offset(video_time: u64, last_frame_ts: u64) -> u64 {
    video_time
        .wrapping_sub(last_frame_ts)
        .wrapping_add(FRAME_TOLERANCE)
        .wrapping_add(TWOX_TOLERANCE)
}

/// `obs_source_deinterlace_pacing_reanchor()`: an unbuffered source's
/// `deinterlace_offset` after it picked the frame at `frame_ts`, from its
/// `offset` before (0 for none). The new offset is `video_time - frame_ts`
/// when there was none, when it is less than `offset` (the frame came
/// early), or when it is more by the larger of the half duration and the
/// canvas interval, less [`TWOX_TOLERANCE`], or more (the frame missed its
/// tick and its first field); `offset` is kept otherwise.
pub fn reanchor(
    offset: u64,
    video_time: u64,
    frame_ts: u64,
    half_duration: u32,
    frame_interval_ns: u64,
) -> u64 {
    let half_duration = u64::from(half_duration);
    let field_or_tick = if half_duration > frame_interval_ns {
        half_duration
    } else {
        frame_interval_ns
    };
    let new_offset = video_time.wrapping_sub(frame_ts);
    // (int64_t)(offset - deinterlace_offset)
    let late = new_offset.wrapping_sub(offset).cast_signed();

    if offset == 0 || late < 0 || late.cast_unsigned().wrapping_add(TWOX_TOLERANCE) >= field_or_tick
    {
        new_offset
    } else {
        offset
    }
}
