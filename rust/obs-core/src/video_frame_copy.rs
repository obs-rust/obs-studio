//! Safe core of `libobs/obs-video-frame-copy.c.h`: the output frame copy of
//! `obs-video.c`, which runs the copy plan of [`video_copy_plan`] into the
//! locked output frame.
//!
//! - [`copy_plane`] (`set_gpu_converted_plane()`): one copy of the plan,
//!   either one copy of [`FrameCopy::bytes`] or [`FrameCopy::rows`] rows of
//!   it at the two strides. It returns how far the source moved on, which is
//!   where the next plane starts in a packed source.
//! - [`steps`] and [`copy_frame`] (`set_gpu_converted_data()`): every copy of
//!   a [`gpu_copy_plan`], in order, up to [`CopyPlan::count`] or
//!   [`COPY_MAX`], whichever is less. Copy `i` writes output plane `i`. It
//!   reads input plane `i`, or, when [`FrameCopy::src_after_plane0`] is set
//!   (the UV plane of a packed NV12 or P010 frame), input plane 0 from where
//!   copy 0's source ended.
//! - [`copy_rgbx_frame`] (`copy_rgbx_frame()`): the one plane of an RGBA,
//!   BGRA or BGRX frame, as [`rgbx_copy_plan`] says.
//!
//! The C copies through raw pointers. Here every copy runs over slices, and
//! [`src_len`] and [`dst_len`] say how many bytes of each a copy reads and
//! writes from its start, so the C shim can make slices of exactly that
//! size from the frame's planes. A slice shorter than its copy needs copies
//! nothing ([`copy_plane`] returns `None`, and [`copy_frame`] stops there);
//! the C would read or write past the plane. Plans `gpu_copy_plan()` and
//! `rgbx_copy_plan()` make never ask for more than the frame's planes hold,
//! so this never happens in libobs.
//!
//! A copy that reads after plane 0 as copy 0 (no plan does) has no source:
//! the C reads from NULL there. It is skipped.
//!
//! [`video_copy_plan`]: crate::video_copy_plan
//! [`gpu_copy_plan`]: crate::video_copy_plan::gpu_copy_plan
//! [`rgbx_copy_plan`]: crate::video_copy_plan::rgbx_copy_plan

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable
)]

use obs_media_io::video_io::VideoFormat;

use crate::video_copy_plan::{COPY_MAX, CopyPlan, FrameCopy, rgbx_copy_plan};

/// `size_t` of a `u32`.
fn size(value: u32) -> Option<usize> {
    usize::try_from(value).ok()
}

/// The bytes from a copy's start that it touches at `stride`: the one copy,
/// or `rows - 1` strides and one row. A copy of no bytes touches none.
fn span(copy: &FrameCopy, stride: u32) -> Option<usize> {
    if copy.bytes == 0 {
        return Some(0);
    }
    if copy.whole {
        return Some(copy.bytes);
    }
    match copy.rows.checked_sub(1) {
        None => Some(0),
        Some(last) => size(last)?
            .checked_mul(size(stride)?)?
            .checked_add(copy.bytes),
    }
}

/// How many bytes of its source `copy` reads, from where it starts. `None`
/// when that overflows `usize`.
#[must_use]
pub fn src_len(copy: &FrameCopy) -> Option<usize> {
    span(copy, copy.src_stride)
}

/// How many bytes of its destination `copy` writes, from where it starts.
/// `None` when that overflows `usize`.
#[must_use]
pub fn dst_len(copy: &FrameCopy) -> Option<usize> {
    span(copy, copy.dst_stride)
}

/// How far `copy` moves its source on: the one copy's bytes, or `rows`
/// source strides (what `set_gpu_converted_plane()` returns, less its `in`).
/// `None` when that overflows `usize`.
#[must_use]
pub fn src_advance(copy: &FrameCopy) -> Option<usize> {
    if copy.whole {
        Some(copy.bytes)
    } else {
        size(copy.rows)?.checked_mul(size(copy.src_stride)?)
    }
}

/// `set_gpu_converted_plane()`: copies `src` into `dst` as `copy` says, and
/// returns how far the source moved on ([`src_advance`]). Copies nothing and
/// returns `None` when `src` is shorter than [`src_len`] or `dst` than
/// [`dst_len`], or when either overflows.
pub fn copy_plane(copy: &FrameCopy, src: &[u8], dst: &mut [u8]) -> Option<usize> {
    let advance = src_advance(copy)?;
    if src.len() < src_len(copy)? || dst.len() < dst_len(copy)? {
        return None;
    }
    if copy.bytes == 0 {
        return Some(advance);
    }
    if copy.whole {
        dst.get_mut(..copy.bytes)?
            .copy_from_slice(src.get(..copy.bytes)?);
    } else {
        let src_stride = size(copy.src_stride)?;
        let dst_stride = size(copy.dst_stride)?;
        let (mut from, mut to) = (0_usize, 0_usize);
        for _ in 0..copy.rows {
            let row = src.get(from..from.checked_add(copy.bytes)?)?;
            dst.get_mut(to..to.checked_add(copy.bytes)?)?
                .copy_from_slice(row);
            // The last row's strides may end past the slices: they are only
            // counted, as the C's pointers are.
            from = from.wrapping_add(src_stride);
            to = to.wrapping_add(dst_stride);
        }
    }
    Some(advance)
}

/// Where a copy of the plan reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Source {
    /// Input plane `i`, from its start.
    Plane(usize),
    /// Input plane 0, from this offset: where copy 0's source ended.
    AfterPlane0(usize),
}

/// One copy of `set_gpu_converted_data()`'s loop.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Step {
    /// The output plane it writes, and its place in the plan.
    pub plane: usize,
    /// The copy.
    pub copy: FrameCopy,
    /// Where it reads; `None` for a copy 0 that reads after plane 0 (the
    /// C's NULL source; no plan has it), or when copy 0's end overflows.
    pub source: Option<Source>,
}

/// `set_gpu_converted_data()`'s loop: the plan's copies in order, up to
/// [`CopyPlan::count`] or [`COPY_MAX`], whichever is less, with where each
/// one reads.
pub fn steps(plan: &CopyPlan) -> impl Iterator<Item = Step> + '_ {
    let after_plane0 = plan.copies.first().and_then(src_advance);
    plan.copies
        .iter()
        .take(plan.count)
        .enumerate()
        .map(move |(plane, copy)| Step {
            plane,
            copy: *copy,
            source: if !copy.src_after_plane0 {
                Some(Source::Plane(plane))
            } else if plane == 0 {
                None
            } else {
                after_plane0.map(Source::AfterPlane0)
            },
        })
}

/// `set_gpu_converted_data()` over slices: runs every [`steps`] copy from
/// `src` into `dst`, and returns how many ran. It stops at the first copy
/// whose slices are too short for it ([`copy_plane`]); a step with no
/// source is skipped.
pub fn copy_frame(
    plan: &CopyPlan,
    src: [&[u8]; COPY_MAX],
    dst: &mut [&mut [u8]; COPY_MAX],
) -> usize {
    let mut ran = 0_usize;
    for step in steps(plan) {
        let from = match step.source {
            None => continue,
            Some(Source::Plane(i)) => src.get(i).copied(),
            Some(Source::AfterPlane0(at)) => src.first().and_then(|s| s.get(at..)),
        };
        let (Some(from), Some(to)) = (from, dst.get_mut(step.plane)) else {
            break;
        };
        if copy_plane(&step.copy, from, to).is_none() {
            break;
        }
        ran = ran.saturating_add(1);
    }
    ran
}

/// `copy_rgbx_frame()` over slices: plane 0 of a `width` x `height` RGBA,
/// BGRA or BGRX frame at `src_linesize` into one at `dst_linesize`, as
/// [`rgbx_copy_plan`] says. `false` (nothing copied) when a slice is too
/// short ([`copy_plane`]).
pub fn copy_rgbx_frame(
    width: u32,
    height: u32,
    src: &[u8],
    src_linesize: u32,
    dst: &mut [u8],
    dst_linesize: u32,
) -> bool {
    let copy = rgbx_copy_plan(width, height, src_linesize, dst_linesize);
    copy_plane(&copy, src, dst).is_some()
}

/// How many of each frame's leading line sizes [`gpu_copy_plan`] reads for
/// `format` (`None` outside `enum video_format`): all three for I420, I444
/// and I010, the first two for NV12, P010, P216 and P416, none for a format
/// with no GPU conversion. The shim reads only these, as the C plan does;
/// the rest stay 0.
///
/// [`gpu_copy_plan`]: crate::video_copy_plan::gpu_copy_plan
#[must_use]
pub fn line_sizes_read(format: Option<VideoFormat>) -> usize {
    match format {
        Some(VideoFormat::I420 | VideoFormat::I444 | VideoFormat::I010) => 3,
        Some(VideoFormat::Nv12 | VideoFormat::P010 | VideoFormat::P216 | VideoFormat::P416) => 2,
        _ => 0,
    }
}
