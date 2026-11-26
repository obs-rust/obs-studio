//! Safe core of `libobs/obs-source-frame-copy-data.c.h`:
//! `obs_source_frame_copy()` and the `copy_frame_data()` it runs, which
//! `cache_video()` and the two preload paths of `obs-source.c.h` call too.
//!
//! [`copy_frame_data`] does what the C does, in its order: write the
//! source's [`Metadata`] into the destination (flip, flags, transfer, range,
//! maximum luminance, timestamp and colour matrix); ask
//! [`frame_copy::copy_plan`] for the copies, with the destination's range
//! (now the source's), height and line sizes; write the source's
//! [`ColorRange`] when the plan says so; then run each plane's copy in
//! order. The decisions are the plan's (no new plan logic here); this module
//! only runs it.
//!
//! The frames are behind [`Frames`], so the core never sees a pointer: the
//! C ABI shim writes the fields and builds each plane's slices from
//! [`plane_extent`], and [`copy_plane`] copies between them as
//! `copy_frame_data_plane()` does: one copy of the plan's bytes when the line
//! sizes are equal, else row `y` from `y * src_stride` into `y * dst_stride`,
//! each product in `u32` as the C's `uint32_t` (wrap for wrap), the smaller
//! line size each. The colour values are the floats' bits: they are copied,
//! never computed.
//!
//! Nothing here allocates, keeps state, takes a lock or panics.

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

use crate::frame_copy::{self, PLANES, PlaneCopy};

/// The fields `copy_frame_data()` always copies, in its order. The colour
/// matrix is the 16 floats' bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Metadata {
    pub flip: bool,
    pub flags: u8,
    pub trc: u8,
    pub full_range: bool,
    pub max_luminance: u16,
    pub timestamp: u64,
    pub color_matrix: [u32; 16],
}

/// `color_range_min` and `color_range_max`, the floats' bits: copied only
/// when the plan says so.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ColorRange {
    pub min: [u32; 3],
    pub max: [u32; 3],
}

/// What `copy_frame_data()` reads of the source frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Source {
    pub metadata: Metadata,
    pub color_range: ColorRange,
    /// An `enum video_format` value, as the C's `int`.
    pub format: i32,
    /// The first [`PLANES`] line sizes.
    pub linesize: [u32; PLANES],
}

/// What `copy_frame_data()` reads of the destination frame's shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Destination {
    pub height: u32,
    /// The first [`PLANES`] line sizes.
    pub linesize: [u32; PLANES],
}

/// The destination and source frames, as `copy_frame_data()` writes them.
pub trait Frames {
    /// Writes the source's metadata into the destination, field by field in
    /// [`Metadata`]'s order.
    fn write_metadata(&mut self, metadata: Metadata);
    /// Writes the source's colour range into the destination.
    fn write_color_range(&mut self, range: ColorRange);
    /// Runs one plane's copy (with [`copy_plane`] over the plane's slices).
    fn copy_plane(&mut self, plane: usize, copy: &PlaneCopy);
}

/// `copy_frame_data()`: the metadata, then the plan of the source's format
/// for the destination's range (the source's, just written), height and
/// line sizes: the colour range when the plan says so, then each planned
/// plane in order.
pub fn copy_frame_data(frames: &mut impl Frames, src: &Source, dst: Destination) {
    frames.write_metadata(src.metadata);
    let plan = frame_copy::copy_plan(
        src.format,
        src.metadata.full_range,
        dst.height,
        src.linesize,
        dst.linesize,
    );
    if plan.color_range {
        frames.write_color_range(src.color_range);
    }
    for (plane, copy) in plan.planes.iter().take(plan.count).enumerate() {
        frames.copy_plane(plane, copy);
    }
}

/// `size_t` of a `u32`.
#[allow(clippy::as_conversions)]
const fn size(value: u32) -> usize {
    value as usize
}

/// The greatest `y * stride` (in `u32`, wrapping) over the rows `0..rows`.
fn last_row(rows: u32, stride: u32) -> u32 {
    let Some(last) = rows.checked_sub(1) else {
        return 0;
    };
    match last.checked_mul(stride) {
        Some(offset) => offset,
        None => (0..rows).map(|y| y.wrapping_mul(stride)).max().unwrap_or(0),
    }
}

/// The bytes of the destination and of the source plane one copy touches,
/// from each plane's start: `(dst, src)`. The plan's bytes for one copy;
/// else up to the end of the row that starts furthest in (in `u32`, as the
/// C computes each row's start); `(0, 0)` when nothing is copied.
#[must_use]
pub fn plane_extent(copy: &PlaneCopy) -> (usize, usize) {
    if copy.whole {
        return (copy.bytes, copy.bytes);
    }
    if copy.bytes == 0 || copy.rows == 0 {
        return (0, 0);
    }
    let end = |stride| size(last_row(copy.rows, stride)).saturating_add(copy.bytes);
    (end(copy.dst_stride), end(copy.src_stride))
}

/// `copy_frame_data_plane()`: one copy of the plan's bytes when the line
/// sizes are equal, else row `y` from `y * src_stride` into `y *
/// dst_stride` (each in `u32`), the plan's bytes each. `dst` and `src` start
/// at the planes' starts and hold [`plane_extent`]'s bytes; a row outside
/// them is not copied (the shim's slices always hold every row).
pub fn copy_plane(dst: &mut [u8], src: &[u8], copy: &PlaneCopy) {
    if copy.whole {
        if let (Some(to), Some(from)) = (dst.get_mut(..copy.bytes), src.get(..copy.bytes)) {
            to.copy_from_slice(from);
        }
        return;
    }
    for y in 0..copy.rows {
        let pos_src = size(y.wrapping_mul(copy.src_stride));
        let pos_dst = size(y.wrapping_mul(copy.dst_stride));
        let (Some(src_end), Some(dst_end)) = (
            pos_src.checked_add(copy.bytes),
            pos_dst.checked_add(copy.bytes),
        ) else {
            continue;
        };
        if let (Some(to), Some(from)) = (dst.get_mut(pos_dst..dst_end), src.get(pos_src..src_end)) {
            to.copy_from_slice(from);
        }
    }
}
