//! C ABI for `libobs/obs-source-frame-copy-data.c.h`:
//! `obs_source_frame_copy()` (`EXPORT` in `obs.h`), which `cache_video()`
//! and the two preload paths of `obs-source.c.h` call too.
//!
//! [`obs_source_frame`] mirrors the public `struct obs_source_frame`. The
//! shim reads what `copy_frame_data()` reads (the source's metadata, colour
//! range, format and first four line sizes; the destination's height and
//! first four line sizes) and hands the writes to
//! [`frame_copy_data::copy_frame_data`] through [`Frames`]: each field is
//! written through the raw pointer, never through a reference to the whole
//! struct (another thread may touch `refs`), and each planned plane gets one
//! slice of the destination's plane and one of the source's, as long as
//! [`plane_extent`] says that copy reaches. `format`, `width`, `height`,
//! `data`, `linesize`, `refs` and `prev_frame` are never written.
//!
//! No intentional differences from C.

use core::ffi::{c_int, c_long};
use core::slice;

use crate::frame_copy::{PLANES, PlaneCopy};
use crate::frame_copy_data::{
    self, ColorRange, Destination, Frames, Metadata, Source, plane_extent,
};

/// `MAX_AV_PLANES` (`media-io/media-io-defs.h`).
pub const MAX_AV_PLANES: usize = 8;

/// `struct obs_source_frame` (`obs.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
#[allow(non_camel_case_types)]
pub struct obs_source_frame {
    pub data: [*mut u8; MAX_AV_PLANES],
    pub linesize: [u32; MAX_AV_PLANES],
    pub width: u32,
    pub height: u32,
    pub timestamp: u64,
    /// `enum video_format`, an `int`.
    pub format: c_int,
    pub color_matrix: [f32; 16],
    pub full_range: bool,
    pub max_luminance: u16,
    pub color_range_min: [f32; 3],
    pub color_range_max: [f32; 3],
    pub flip: bool,
    pub flags: u8,
    /// `enum video_trc`.
    pub trc: u8,
    /// `volatile long`.
    pub refs: c_long,
    pub prev_frame: bool,
}

fn bits<const N: usize>(floats: [f32; N]) -> [u32; N] {
    floats.map(f32::to_bits)
}

fn floats<const N: usize>(bits: [u32; N]) -> [f32; N] {
    bits.map(f32::from_bits)
}

/// The first [`PLANES`] of a frame's line sizes.
fn lines(linesize: [u32; MAX_AV_PLANES]) -> [u32; PLANES] {
    let mut lines = [0; PLANES];
    for (line, size) in lines.iter_mut().zip(linesize) {
        *line = size;
    }
    lines
}

/// The two frames, behind their raw pointers.
struct RawFrames {
    dst: *mut obs_source_frame,
    src: *const obs_source_frame,
}

impl Frames for RawFrames {
    fn write_metadata(&mut self, m: Metadata) {
        let dst = self.dst;
        // SAFETY: `obs_source_frame_copy`'s caller passes a valid, writable
        // destination; each field is written in place, the C's order.
        unsafe {
            (*dst).flip = m.flip;
            (*dst).flags = m.flags;
            (*dst).trc = m.trc;
            (*dst).full_range = m.full_range;
            (*dst).max_luminance = m.max_luminance;
            (*dst).timestamp = m.timestamp;
            (*dst).color_matrix = floats(m.color_matrix);
        }
    }

    fn write_color_range(&mut self, range: ColorRange) {
        let dst = self.dst;
        // SAFETY: as in `write_metadata`.
        unsafe {
            (*dst).color_range_min = floats(range.min);
            (*dst).color_range_max = floats(range.max);
        }
    }

    fn copy_plane(&mut self, plane: usize, copy: &PlaneCopy) {
        let (dst_len, src_len) = plane_extent(copy);
        if dst_len == 0 || src_len == 0 {
            return;
        }
        // SAFETY: both frames are valid (the caller's contract); the data
        // arrays are read by value.
        let (to, from) = unsafe { ((*self.dst).data, (*self.src).data) };
        let (Some(&to), Some(&from)) = (to.get(plane), from.get(plane)) else {
            return;
        };
        // SAFETY: the caller's contract: each plane the plan copies holds
        // the bytes the plan reaches in both frames, and the two frames'
        // planes do not overlap (as `memcpy()` requires).
        let (to, from) = unsafe {
            (
                slice::from_raw_parts_mut(to, dst_len),
                slice::from_raw_parts(from, src_len),
            )
        };
        frame_copy_data::copy_plane(to, from, copy);
    }
}

/// `void obs_source_frame_copy(struct obs_source_frame *dst, const struct
/// obs_source_frame *src)`: `copy_frame_data(dst, src)`.
///
/// # Safety
///
/// `dst` and `src` point to valid frames, `dst` writable. For each plane
/// the copy plan of the source's format and the destination's height and
/// line sizes copies, the planes hold the bytes it reaches, and the two
/// frames' planes do not overlap: what the C's `memcpy()`s need.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_frame_copy(
    dst: *mut obs_source_frame,
    src: *const obs_source_frame,
) {
    // SAFETY: the caller passes two valid frames; each field is read by
    // value through the pointer, with no reference to the whole struct.
    let (source, destination) = unsafe {
        (
            Source {
                metadata: Metadata {
                    flip: (*src).flip,
                    flags: (*src).flags,
                    trc: (*src).trc,
                    full_range: (*src).full_range,
                    max_luminance: (*src).max_luminance,
                    timestamp: (*src).timestamp,
                    color_matrix: bits((*src).color_matrix),
                },
                color_range: ColorRange {
                    min: bits((*src).color_range_min),
                    max: bits((*src).color_range_max),
                },
                format: (*src).format,
                linesize: lines((*src).linesize),
            },
            Destination {
                height: (*dst).height,
                linesize: lines((*dst).linesize),
            },
        )
    };
    frame_copy_data::copy_frame_data(&mut RawFrames { dst, src }, &source, destination);
}
