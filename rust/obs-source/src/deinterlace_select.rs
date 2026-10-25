//! Safe core of `libobs/obs-source-deinterlace-select.c.h`: the frame
//! selection of a deinterlaced async source, from `first_frame()`,
//! `ready_deinterlace_frames()`, `deinterlace_get_closest_frames()` and
//! `deinterlace_update_async_video()`.
//!
//! Not ported yet: every function is `todo!()`, so the Tier 1 tests fail.

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
    let _ = (last_frame_ts, count);
    todo!("obs_source_deinterlace_select_first")
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

/// `obs_source_deinterlace_select_walk()`: `ready_deinterlace_frames()`
/// over the queued timestamps, oldest first, at the canvas time
/// `sys_time`. An empty queue drops and takes nothing and keeps
/// `last_frame_ts`.
pub fn walk(timestamps: &[u64], sys_time: u64, state: State) -> Walk {
    let _ = (timestamps, sys_time, state);
    todo!("obs_source_deinterlace_select_walk")
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
/// its end ([`crate::deinterlace_pacing::wait_for_next`]), from
/// [`crate::deinterlace_pacing::waiting_frame_ts`]; otherwise it pairs with the
/// frame at `next_timestamp`. `offset` is 0 when the source read
/// `async_unbuffered` as false again.
pub fn unbuffered_pair(timestamp: u64, next_timestamp: u64, offset: u64, sys_time: u64) -> Pair {
    let _ = (timestamp, next_timestamp, offset, sys_time);
    todo!("obs_source_deinterlace_select_unbuffered_pair")
}

/// `obs_source_deinterlace_select_take_reads_flag()`: with frames left
/// after the taken one, its `prev_frame` flag is read.
pub fn take_reads_flag(remaining: usize) -> bool {
    let _ = remaining;
    todo!("obs_source_deinterlace_select_take_reads_flag")
}

/// `obs_source_deinterlace_select_take_pair()`: the taken frame becomes the
/// previous frame and the next one the current frame.
pub fn take_pair(remaining: usize, prev_frame: bool) -> bool {
    let _ = (remaining, prev_frame);
    todo!("obs_source_deinterlace_select_take_pair")
}

/// `obs_source_deinterlace_select_take_init_last()`: `last_frame_ts`
/// starts at the current frame.
pub fn take_init_last(last_frame_ts: u64) -> bool {
    let _ = last_frame_ts;
    todo!("obs_source_deinterlace_select_take_init_last")
}

/// `enum obs_source_deinterlace_select_offset_kind`: how
/// `deinterlace_offset` is set after a take.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum OffsetKind {
    /// `OBS_SOURCE_DEINTERLACE_SELECT_OFFSET_BUFFERED`:
    /// [`crate::deinterlace_pacing::buffered_offset`].
    Buffered = 0,
    /// `OBS_SOURCE_DEINTERLACE_SELECT_OFFSET_REANCHOR`:
    /// [`crate::deinterlace_pacing::reanchor`].
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
    let _ = async_unbuffered;
    todo!("obs_source_deinterlace_select_take_offset")
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
    let _ = (has_prev, updated);
    todo!("obs_source_deinterlace_select_update")
}
