//! Safe core of `libobs/obs-source-async-pacing.c.h`: the async video frame
//! pacing of `get_closest_frame()` and `ready_async_frame()`.
//!
//! Once a canvas tick, `async_tick()` asks which queued frame of an async
//! source without 2x deinterlacing (media, cameras, capture cards) to show.
//! [`pace`] answers from the queue's timestamps, oldest first, and the
//! source's clock: how many frames to drop from the front, whether to take
//! the front frame after that, and the new `last_frame_ts`.
//!
//! `obs-source.c.h` keeps everything that touches the queue or the frames:
//! `async_frames`, `da_erase()`, `remove_async_frame()`, the async cache,
//! `async_mutex` and the frame pointers. It copies the timestamps in and
//! applies the result. Nothing here keeps state, takes a lock or calls
//! libobs.

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

/// What `get_closest_frame()` does with the queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Pacing {
    /// Frames to erase from the front, each with `remove_async_frame()`.
    pub drop_front: usize,
    /// Then take the front frame as `cur_async_frame`.
    pub take_first: bool,
    /// The new `source->last_frame_ts`.
    pub last_frame_ts: u64,
}

/// `frame_out_of_bounds()` (`obs-internal.h`), on values.
pub fn frame_out_of_bounds(last_frame_ts: u64, ts: u64) -> bool {
    let _ = (last_frame_ts, ts);
    todo!()
}

/// `obs_source_async_pacing()`: the pacing of the queue `timestamps`,
/// oldest first, at the canvas time `sys_time`.
pub fn pace(timestamps: &[u64], sys_time: u64, state: State) -> Pacing {
    let _ = (timestamps, sys_time, state);
    todo!()
}
