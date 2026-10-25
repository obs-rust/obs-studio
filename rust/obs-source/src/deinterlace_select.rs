//! Safe core of `libobs/obs-source-deinterlace-select.c.h`: the frame
//! selection of a deinterlaced async source, from `first_frame()`,
//! `ready_deinterlace_frames()`, `deinterlace_get_closest_frames()` and
//! `deinterlace_update_async_video()`.
//!
//! - [`first`]: a source with no `last_frame_ts` yet takes its front frame,
//!   paired with the next one when two or more are queued;
//! - [`walk`]: the queue walk, on the queued timestamps. An unbuffered
//!   source drops all but two frames. A buffered one resets on a front
//!   frame more than [`MAX_TS_VAR`] from its clock; otherwise it moves its
//!   clock by the canvas time since the last tick and walks the queue:
//!   it stops within [`FRAME_TOLERANCE`] of the next frame, drops the front
//!   frame each step behind a taken pair, stops with two frames or fewer
//!   left and shows the front one alone (#2009), and re-anchors its clock
//!   on a step of more than [`MAX_TS_VAR`] between frames;
//! - [`unbuffered_pair`]: the "don't skip ahead" guard of an unbuffered
//!   source left with two frames, through
//!   [`deinterlace_pacing::wait_for_next`] and
//!   [`deinterlace_pacing::waiting_frame_ts`];
//! - [`take_reads_flag`], [`take_pair`], [`take_init_last`] and
//!   [`take_offset`]: what `deinterlace_get_closest_frames()` does with the
//!   frame it takes;
//! - [`update`]: upload the previous frame, swap the textures, or neither.
//!
//! `obs-source-deinterlace.c.h` keeps `async_mutex`, the `async_frames`
//! queue, its erases, `remove_async_frame()` and the cache, every
//! `prev_frame` write, the frame pointers and the textures. It copies the
//! queued timestamps, reads each of the source's fields where it always
//! did, and applies what comes back: the dropped frames off the front, each
//! erased and then released, then `last_frame_ts`, `deinterlace_offset`,
//! `deinterlace_frame_ts` and the front frame's flag. Nothing here keeps
//! state, takes a lock or calls libobs.
//!
//! Every sum and difference wraps, as `uint64_t` does in the C; nothing
//! divides. Characterized, not endorsed (`test_deinterlace_select_steps`),
//! and kept: a timestamp going backwards is a step of almost 2^64 and
//! re-anchors the clock; `sys_time` before `last_sys_timestamp` moves the
//! clock back; and the compares do not wrap, so frames past `u64::MAX` are
//! far behind a clock that has not wrapped.

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

use crate::deinterlace_pacing;

pub use crate::deinterlace_pacing::FRAME_TOLERANCE;

/// `MAX_TS_VAR` (`obs-internal.h`): a front frame more than this from the
/// source clock is out of bounds, and a step of more than this between
/// queued frames re-anchors the clock. In ns.
pub const MAX_TS_VAR: u64 = 2_000_000_000;

/// What `first_frame()` does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct First {
    /// No `last_frame_ts` yet: take the front frame without the walk.
    pub first: bool,
    /// Set `prev_frame` on the front frame.
    pub flag_front: bool,
}

/// `obs_source_deinterlace_select_first()`: `first_frame()` of a source
/// whose `last_frame_ts` is `last_frame_ts`, with `count` frames queued.
pub fn first(last_frame_ts: u64, count: usize) -> First {
    let first = last_frame_ts == 0;
    First {
        first,
        flag_front: first && count >= 2,
    }
}

/// The source's values the walk reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct State {
    /// `last_frame_ts`, the source clock (ignored when unbuffered).
    pub last_frame_ts: u64,
    /// `last_sys_timestamp`, the canvas time of the last tick.
    pub last_sys_timestamp: u64,
    /// `async_unbuffered`.
    pub async_unbuffered: bool,
}

/// What `ready_deinterlace_frames()` does with the queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Walk {
    /// Frames to erase from the front, each released to the cache.
    pub drop_front: usize,
    /// The new `last_frame_ts`.
    pub last_frame_ts: u64,
    /// Set `deinterlace_offset` to 0.
    pub offset_reset: bool,
    /// Unbuffered, two frames left: [`unbuffered_pair`] decides the front
    /// frame's flag.
    pub pairs: bool,
    /// Buffered: set `prev_frame` on the front frame.
    pub flag_front: bool,
    /// Take the front frame.
    pub ready: bool,
}

/// `frame_out_of_bounds()` of `obs-internal.h`, on values.
fn out_of_bounds(last_frame_ts: u64, ts: u64) -> bool {
    if ts < last_frame_ts {
        last_frame_ts.wrapping_sub(ts) > MAX_TS_VAR
    } else {
        ts.wrapping_sub(last_frame_ts) > MAX_TS_VAR
    }
}

/// `obs_source_deinterlace_select_walk()`: `ready_deinterlace_frames()`
/// over the queued timestamps, oldest first, at the canvas time
/// `sys_time`. An empty queue drops and takes nothing and keeps
/// `last_frame_ts`.
pub fn walk(timestamps: &[u64], sys_time: u64, state: State) -> Walk {
    let mut result = Walk {
        drop_front: 0,
        last_frame_ts: state.last_frame_ts,
        offset_reset: false,
        pairs: false,
        flag_front: false,
        ready: false,
    };
    let Some(&front) = timestamps.first() else {
        return result;
    };

    if state.async_unbuffered {
        // All but two frames go; the front one left is the clock.
        let drop_front = timestamps.len().saturating_sub(2);
        result.drop_front = drop_front;
        result.pairs = timestamps.len().saturating_sub(drop_front) == 2;
        result.last_frame_ts = timestamps.get(drop_front).copied().unwrap_or(front);
        result.ready = true;
        return result;
    }

    // account for timestamp invalidation
    if out_of_bounds(result.last_frame_ts, front) {
        result.last_frame_ts = front;
        result.offset_reset = true;
        result.ready = true;
        return result;
    }

    let sys_offset = sys_time.wrapping_sub(state.last_sys_timestamp);
    let mut frame_time = front;
    let mut frame_offset = frame_time.wrapping_sub(result.last_frame_ts);
    result.last_frame_ts = result.last_frame_ts.wrapping_add(sys_offset);

    // The queue is timestamps[first..]; each erase of the C is first += 1.
    let mut first = 0_usize;
    let mut next_frame = front;
    let mut prev_frame = false;
    let mut frame = false;

    while result.last_frame_ts > next_frame {
        // Keep the frame while the next is due within the tolerance.
        if result.last_frame_ts.wrapping_sub(next_frame) < FRAME_TOLERANCE {
            break;
        }

        if prev_frame {
            first = first.saturating_add(1);
        }

        let num = timestamps.len().saturating_sub(first);
        // Two frames or fewer left: stop and show the front one alone,
        // unless none was taken yet. A frame just dropped is no pair for
        // it (#2009).
        if num <= 2 && (frame || num != 2) {
            result.drop_front = first;
            result.offset_reset = true;
            result.ready = true;
            return result;
        }

        let idx = if frame { 2 } else { 1 };
        prev_frame = frame;
        frame = true;
        // num > 2, or num == 2 with idx 1: the index is in the queue.
        let Some(&next) = timestamps.get(first.saturating_add(idx)) else {
            break;
        };
        next_frame = next;

        // more timestamp checking and compensating
        if next_frame.wrapping_sub(frame_time) > MAX_TS_VAR {
            result.last_frame_ts = next_frame.wrapping_sub(frame_offset);
            result.offset_reset = true;
        }

        frame_time = next_frame;
        frame_offset = frame_time.wrapping_sub(result.last_frame_ts);
    }

    result.drop_front = first;
    result.flag_front = prev_frame;
    result.ready = frame;
    result
}

/// What an unbuffered source left with two frames does with the front one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pair {
    /// The front frame's `prev_frame`: it pairs with the next frame.
    pub prev_frame: bool,
    /// The new `deinterlace_frame_ts` while the source waits, stored before
    /// the flag.
    pub frame_ts: Option<u64>,
}

/// `obs_source_deinterlace_select_unbuffered_pair()`: the front frame at
/// `timestamp` waits alone while an offset is set and `sys_time` is before
/// its end ([`deinterlace_pacing::wait_for_next`]), from
/// [`deinterlace_pacing::waiting_frame_ts`]; otherwise it pairs with the
/// frame at `next_timestamp`. `offset` is 0 when the source read
/// `async_unbuffered` as false again.
pub fn unbuffered_pair(timestamp: u64, next_timestamp: u64, offset: u64, sys_time: u64) -> Pair {
    if offset != 0 && deinterlace_pacing::wait_for_next(timestamp, next_timestamp, offset, sys_time)
    {
        // Don't skip ahead prematurely.
        Pair {
            prev_frame: false,
            frame_ts: Some(deinterlace_pacing::waiting_frame_ts(
                timestamp,
                next_timestamp,
            )),
        }
    } else {
        Pair {
            prev_frame: true,
            frame_ts: None,
        }
    }
}

/// `obs_source_deinterlace_select_take_reads_flag()`: with frames left
/// after the taken one, its `prev_frame` flag is read.
pub fn take_reads_flag(remaining: usize) -> bool {
    remaining > 0
}

/// `obs_source_deinterlace_select_take_pair()`: the taken frame becomes the
/// previous frame and the next one the current frame.
pub fn take_pair(remaining: usize, prev_frame: bool) -> bool {
    remaining > 0 && prev_frame
}

/// `obs_source_deinterlace_select_take_init_last()`: `last_frame_ts`
/// starts at the current frame.
pub fn take_init_last(last_frame_ts: u64) -> bool {
    last_frame_ts == 0
}

/// `enum obs_source_deinterlace_select_offset_kind`: how
/// `deinterlace_offset` is set after a take.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum OffsetKind {
    /// `OBS_SOURCE_DEINTERLACE_SELECT_OFFSET_BUFFERED`:
    /// [`deinterlace_pacing::buffered_offset`].
    Buffered = 0,
    /// `OBS_SOURCE_DEINTERLACE_SELECT_OFFSET_REANCHOR`:
    /// [`deinterlace_pacing::reanchor`].
    Reanchor = 1,
}

impl From<OffsetKind> for u32 {
    /// The C enum's value.
    fn from(kind: OffsetKind) -> Self {
        match kind {
            OffsetKind::Buffered => 0,
            OffsetKind::Reanchor => 1,
        }
    }
}

/// `obs_source_deinterlace_select_take_offset()`.
pub fn take_offset(async_unbuffered: bool) -> OffsetKind {
    if async_unbuffered {
        OffsetKind::Reanchor
    } else {
        OffsetKind::Buffered
    }
}

/// `enum obs_source_deinterlace_select_update_step`: what
/// `deinterlace_update_async_video()` does with the textures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum Update {
    /// `OBS_SOURCE_DEINTERLACE_SELECT_UPDATE_NONE`.
    None = 0,
    /// `OBS_SOURCE_DEINTERLACE_SELECT_UPDATE_UPLOAD`: upload the previous
    /// frame into the previous textures.
    Upload = 1,
    /// `OBS_SOURCE_DEINTERLACE_SELECT_UPDATE_SWAP`: swap the current and
    /// previous textures.
    Swap = 2,
}

impl From<Update> for u32 {
    /// The C enum's value.
    fn from(update: Update) -> Self {
        match update {
            Update::None => 0,
            Update::Upload => 1,
            Update::Swap => 2,
        }
    }
}

/// `obs_source_deinterlace_select_update()`: upload with a previous frame,
/// else swap when the current frame was updated, else nothing.
pub fn update(has_prev: bool, updated: bool) -> Update {
    if has_prev {
        Update::Upload
    } else if updated {
        Update::Swap
    } else {
        Update::None
    }
}
