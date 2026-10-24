//! Safe core of `libobs/obs-source-frame-copy.c.h`: the async frame copy
//! plan and cache decisions of `obs-source.c`.
//!
//! Every frame an async source outputs (`obs_source_output_video()`,
//! `obs_source_preload_video()`, `obs_source_set_video_frame()` and their
//! `*2()` variants) is copied into a frame of the source's cache. This
//! module decides, on plain values:
//! - the frame's range ([`full_range`], [`full_range2`]): a frame that is
//!   not YUV is full range; for the `*2()` variants, full range is what
//!   `resolve_video_range()` gives;
//! - whether the cache is kept, reset or the frame refused ([`cache_step`]:
//!   the queue limit at [`MAX_ASYNC_FRAMES`], then [`texture_changed`]);
//! - what one entry of `clean_cache()` does ([`clean`]: an unused entry
//!   counts one more call and is dropped at [`MAX_UNUSED`]);
//! - what `copy_frame_data()` copies ([`copy_plan`]): the colour range when
//!   the frame is not full range, then each plane of the format
//!   ([`plane_rows`]), one copy of line size x rows when the two line sizes
//!   are equal, else rows of the smaller line size ([`plane_copy`]);
//! - whether a preload frame must be made again ([`preload_changed`]).
//!
//! `obs-source.c.h` keeps every `memcpy()` (it runs the plan), the metadata
//! copy, `obs_source_frame_create()` and `_destroy()`, the `async_cache` and
//! `async_frames` darrays and the walk that picks the first unused entry,
//! `async_mutex`, the refcount atomics and `get_convert_type()` (it passes
//! the two convert types).
//!
//! Frames come from capture devices, decoded media and plugins, so any
//! format, height and line size can arrive. Formats and ranges are the C's
//! raw `int`s: a value outside the enum takes the C's paths (no plane, not
//! YUV). The arithmetic is the C's, wrap for wrap: `(height + 1) / 2` wraps
//! in `u32` (0 chroma rows at `u32::MAX`), and the one copy's size is a
//! `usize` product (`size_t`). The plan drives `memcpy()`, so it must be
//! exactly the C's: a row too many overruns a buffer. Nothing here panics,
//! allocates, keeps state or takes a lock.

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

use core::ffi::c_long;

pub use crate::conversion_plan::video_format;

/// `OBS_SOURCE_FRAME_COPY_PLANES`: the planes a plan can copy.
pub const PLANES: usize = 4;

/// `MAX_ASYNC_FRAMES`: the queued frames at which `cache_video()` clears the
/// cache and refuses the frame.
pub const MAX_ASYNC_FRAMES: usize = 30;

/// `MAX_UNUSED_FRAME_DURATION`: the `clean_cache()` calls after which an
/// unused cache entry is dropped.
pub const MAX_UNUSED: c_long = 5;

/// `enum video_range_type` (`media-io/video-io.h`), as the C's `int`.
pub mod video_range {
    pub const DEFAULT: i32 = 0;
    pub const PARTIAL: i32 = 1;
    pub const FULL: i32 = 2;
}

/// The copy of one plane, from the start of the source plane into the start
/// of the destination plane (`struct obs_source_frame_plane_copy`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PlaneCopy {
    /// The two line sizes are equal: one `memcpy()` of `bytes`.
    pub whole: bool,
    /// `whole`: the line size times `rows`, as `size_t`. Otherwise the bytes
    /// of each row, the smaller line size.
    pub bytes: usize,
    /// The rows of the plane. Not `whole`: row `y` is copied from
    /// `y * src_stride` into `y * dst_stride`, each product in `u32` (the
    /// C's `uint32_t`, so it wraps).
    pub rows: u32,
    /// The source's line size.
    pub src_stride: u32,
    /// The destination's line size.
    pub dst_stride: u32,
}

/// The copies of `copy_frame_data()` (`struct obs_source_frame_copy_plan`):
/// copy `i` is plane `i`, in order, after the colour range when
/// `color_range` is set. Planes from `count` on are zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CopyPlan {
    /// Copy `color_range_min` and `color_range_max`: the frame is not full
    /// range.
    pub color_range: bool,
    /// The planes to copy, at most [`PLANES`].
    pub count: usize,
    pub planes: [PlaneCopy; PLANES],
}

/// How each plane's rows follow the frame's height.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rows {
    /// The height.
    Full,
    /// `(height + 1) / 2`: 4:2:0 chroma.
    Half,
}

/// What `async_texture_changed()` compares (`struct
/// obs_source_frame_copy_key`): a size and a convert type (an `enum
/// convert_type` value).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Key {
    pub width: u32,
    pub height: u32,
    pub convert_type: i32,
}

/// What `cache_video()` does before it picks a frame (`enum
/// obs_source_frame_cache_step`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheStep {
    /// Keep the cache: its size and convert type match the frame's.
    Keep,
    /// `free_async_cache()`, then take the frame's width and height.
    Reset,
    /// `free_async_cache()`, `last_frame_ts = 0` and no frame: the queue is
    /// full.
    Overflow,
}

/// One entry of `clean_cache()` (`struct obs_source_frame_cache_age`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheAge {
    /// A used entry: nothing is stored.
    InUse,
    /// An unused entry: store its count after this call.
    Counted(c_long),
    /// An unused entry that reached [`MAX_UNUSED`]: store the count, then
    /// destroy its frame and erase it.
    Drop(c_long),
}

/// What `preload_frame_changed()` compares of a frame (`struct
/// obs_source_frame_copy_shape`): an `enum video_format` value and the size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Shape {
    pub format: i32,
    pub width: u32,
    pub height: u32,
}

/// `(height + 1) / 2` in `u32`: 4:2:0 chroma rows, wrapping to 0 at
/// `u32::MAX` as the C's `uint32_t` does.
#[must_use]
pub const fn half(height: u32) -> u32 {
    let _ = height;
    todo!()
}

/// `copy_frame_data()`'s switch: the planes of `format` and how each one's
/// rows follow the height. P216, P416 and values outside the enum have none.
#[must_use]
pub const fn plane_rows(format: i32) -> &'static [Rows] {
    let _ = format;
    todo!()
}

/// `copy_frame_data_plane()` and `copy_frame_data_line()`: one copy of
/// `dst * rows` (in `size_t`) when the line sizes are equal, else `rows` rows
/// of the smaller line size.
#[must_use]
pub const fn plane_copy(rows: u32, src: u32, dst: u32) -> PlaneCopy {
    let _ = (rows, src, dst);
    todo!()
}

/// `obs_source_frame_copy_plan()`: the copies from a frame of `format` and
/// range `full_range` into one `height` rows high, with the source's line
/// sizes `src` and the destination's `dst`.
#[must_use]
pub fn copy_plan(
    format: i32,
    full_range: bool,
    height: u32,
    src: [u32; PLANES],
    dst: [u32; PLANES],
) -> CopyPlan {
    let _ = (format, full_range, height, src, dst);
    todo!()
}

/// `async_texture_changed()`: the size or the convert type differs.
#[must_use]
pub const fn texture_changed(cache: Key, frame: Key) -> bool {
    let _ = (cache, frame);
    todo!()
}

/// `cache_video()`: `queued` is `async_frames.num`. The queue limit comes
/// first, then [`texture_changed`].
#[must_use]
pub const fn cache_step(queued: usize, cache: Key, frame: Key) -> CacheStep {
    let _ = (queued, cache, frame);
    todo!()
}

/// One entry of `clean_cache()`: an unused entry counts one more call
/// (`++unused_count`) and is dropped when the count reaches [`MAX_UNUSED`].
/// The increment wraps; the C's `long` would overflow at `c_long::MAX`,
/// which no entry reaches (counts start at 0 and the entry is erased at 5).
#[must_use]
pub const fn clean(used: bool, unused_count: c_long) -> CacheAge {
    let _ = (used, unused_count);
    todo!()
}

/// `format_is_yuv()` (`media-io/video-io.h`): false outside the enum.
#[must_use]
pub const fn is_yuv(format: i32) -> bool {
    let _ = format;
    todo!()
}

/// `resolve_video_range()` (`media-io/video-io.h`): `DEFAULT` is partial
/// for YUV and full otherwise; any other value is kept.
#[must_use]
pub const fn resolve_range(format: i32, range: i32) -> i32 {
    let _ = (format, range);
    todo!()
}

/// `obs_source_output_video()`, `obs_source_preload_video()` and
/// `obs_source_set_video_frame()`: a YUV frame keeps `full_range`, any other
/// is full range.
#[must_use]
pub const fn full_range(format: i32, full_range: bool) -> bool {
    let _ = (format, full_range);
    todo!()
}

/// The `*2()` variants: full range exactly when [`resolve_range`] gives
/// `FULL`.
#[must_use]
pub const fn full_range2(format: i32, range: i32) -> bool {
    let _ = (format, range);
    todo!()
}

/// `preload_frame_changed()`: there is no preload frame, or its size or
/// format differs from the new frame's.
#[must_use]
pub const fn preload_changed(preload: Option<Shape>, frame: Shape) -> bool {
    let _ = (preload, frame);
    todo!()
}
