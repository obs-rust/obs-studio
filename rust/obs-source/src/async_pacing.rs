//! Safe core of `libobs/obs-source-async-pacing.c.h`: the async video frame
//! pacing of `get_closest_frame()` and `ready_async_frame()`.
//!
//! Once a canvas tick, `async_tick()` asks which queued frame of an async
//! source without 2x deinterlacing (media, cameras, capture cards) to show.
//! [`pace`] answers from the queue's timestamps, oldest first, and the
//! source's clock: how many frames to drop from the front, whether to take
//! the front frame after that, and the new `last_frame_ts`. The rules:
//! - the first-frame rule: with `last_frame_ts` 0 (no frame shown since the
//!   queue was reset), the oldest frame, buffered or not;
//! - unbuffered: the newest frame, the older ones dropped;
//! - buffered, after a timestamp jump ([`frame_out_of_bounds`] of the oldest
//!   frame): that frame, which becomes the clock;
//! - buffered otherwise, the catch-up walk: the source clock advances by the
//!   canvas time since the last tick and drops the frames it passed. It
//!   keeps a passed frame while the next is due within [`FRAME_TOLERANCE`]
//!   (no needless duplicates; only once a frame is passed), never drops the
//!   last queued frame, and re-anchors when the next frame jumps more than
//!   [`MAX_TS_VAR`] from the one passed. With no frame passed, nothing new;
//! - then, when a frame is shown and `last_frame_ts` is 0, it becomes that
//!   frame's timestamp.
//!
//! `obs-source.c.h` keeps everything that touches the queue or the frames:
//! `async_frames`, `da_erase()`, `remove_async_frame()`, the async cache,
//! `async_mutex` and the frame pointers. It copies the timestamps in,
//! erases [`Pacing::drop_front`] frames from the front (each erase at index
//! 0, as the C's always were), stores [`Pacing::last_frame_ts`] and takes
//! the front frame when [`Pacing::take_first`] is set. Nothing here keeps
//! state, takes a lock or calls libobs.
//!
//! Every sum and difference wraps, as `uint64_t` does in the C.
//! Characterized, not endorsed (`test_async_frame_pacing_steps`), and kept:
//! a timestamp before the passed frame's wraps past [`MAX_TS_VAR`] and
//! re-anchors; a canvas clock going backwards wraps `sys_offset` and moves
//! the source clock back (below 0, to near `u64::MAX`); a re-anchoring tick
//! loses its `sys_offset`; a frame at timestamp 0 seeds `last_frame_ts` 0.

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

/// `MAX_TS_VAR` (`obs-internal.h`): a timestamp more than this from the
/// source clock is a jump. In ns.
pub const MAX_TS_VAR: u64 = 2_000_000_000;

/// The duplicate guard of the catch-up walk: a passed frame is kept while
/// the next is due within this. In ns.
pub const FRAME_TOLERANCE: u64 = 2_000_000;

/// The source's fields that the pacing reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct State {
    /// `source->last_frame_ts`: the source clock, 0 when no frame was shown
    /// since the queue was reset.
    pub last_frame_ts: u64,
    /// `source->last_sys_timestamp`: the canvas time of the last tick.
    pub last_sys_timestamp: u64,
    /// `source->async_unbuffered`.
    pub async_unbuffered: bool,
}

/// What `get_closest_frame()` does with the queue. `drop_front` is below
/// the queue's length (the last queued frame is never dropped), and is 0
/// unless `take_first` is set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Pacing {
    /// Frames to erase from the front, each with `remove_async_frame()`.
    pub drop_front: usize,
    /// Then take the front frame as `cur_async_frame`.
    pub take_first: bool,
    /// The new `source->last_frame_ts`.
    pub last_frame_ts: u64,
}

/// `frame_out_of_bounds()` (`obs-internal.h`), on values: whether `ts` is
/// more than [`MAX_TS_VAR`] from `last_frame_ts`, either way. The C inline
/// stays for `obs-source-deinterlace.c.h`.
pub fn frame_out_of_bounds(last_frame_ts: u64, ts: u64) -> bool {
    if ts < last_frame_ts {
        last_frame_ts.wrapping_sub(ts) > MAX_TS_VAR
    } else {
        ts.wrapping_sub(last_frame_ts) > MAX_TS_VAR
    }
}

/// `obs_source_async_pacing()`: the pacing of the queue `timestamps`,
/// oldest first, at the canvas time `sys_time`. An empty queue drops and
/// takes nothing and keeps `last_frame_ts`.
pub fn pace(timestamps: &[u64], sys_time: u64, state: State) -> Pacing {
    let mut result = Pacing {
        drop_front: 0,
        take_first: false,
        last_frame_ts: state.last_frame_ts,
    };
    let Some(&oldest) = timestamps.first() else {
        return result;
    };

    if state.last_frame_ts == 0 || ready(&mut result, timestamps, oldest, sys_time, state) {
        result.take_first = true;
        if result.last_frame_ts == 0 {
            // drop_front is below the length, so this is the frame taken.
            result.last_frame_ts = timestamps.get(result.drop_front).copied().unwrap_or(oldest);
        }
    }
    result
}

/// `ready_async_frame()`, for a queue whose oldest timestamp is `oldest`.
/// `front` is the index of `async_frames.array[0]`: each `da_erase(..., 0)`
/// of the C is `front += 1`.
fn ready(
    result: &mut Pacing,
    timestamps: &[u64],
    oldest: u64,
    sys_time: u64,
    state: State,
) -> bool {
    if state.async_unbuffered {
        // Drop all but the newest.
        result.drop_front = timestamps.len().saturating_sub(1);
        result.last_frame_ts = timestamps.last().copied().unwrap_or(oldest);
        return true;
    }

    let sys_offset = sys_time.wrapping_sub(state.last_sys_timestamp);
    let mut next_frame = oldest;
    let mut frame_time = oldest;

    // Account for timestamp invalidation.
    if frame_out_of_bounds(result.last_frame_ts, frame_time) {
        result.last_frame_ts = next_frame;
        return true;
    }
    let mut frame_offset = frame_time.wrapping_sub(result.last_frame_ts);
    result.last_frame_ts = result.last_frame_ts.wrapping_add(sys_offset);

    // Whether the walk has passed a frame (the C's frame != NULL); that
    // frame is timestamps[front] and the next is timestamps[front + 1].
    let mut passed = false;
    let mut front: usize = 0;
    while result.last_frame_ts > next_frame {
        // Reduce needless duplication: keep the passed frame while the next
        // is due within the tolerance.
        if passed && result.last_frame_ts.wrapping_sub(next_frame) < FRAME_TOLERANCE {
            break;
        }
        if passed {
            front = front.wrapping_add(1);
        }
        // One frame left: show it.
        let Some(&after) = timestamps.get(front.wrapping_add(1)) else {
            result.drop_front = front;
            return true;
        };
        passed = true;
        next_frame = after;

        // More timestamp checking and compensating.
        if next_frame.wrapping_sub(frame_time) > MAX_TS_VAR {
            result.last_frame_ts = next_frame.wrapping_sub(frame_offset);
        }
        frame_time = next_frame;
        frame_offset = frame_time.wrapping_sub(result.last_frame_ts);
    }

    result.drop_front = front;
    passed
}
