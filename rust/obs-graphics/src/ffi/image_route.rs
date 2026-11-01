//! C ABI for `libobs/graphics/graphics-ffmpeg-route.c.h`, the pixel route
//! of `graphics-ffmpeg.c`, declared in
//! `libobs/graphics/graphics-ffmpeg-route.h` without `EXPORT`.
//!
//! `graphics-ffmpeg.c` (still C) asks for the plan of each decoded frame
//! and switches on its route; it allocates every output with `bmalloc()`
//! (not zeroed), runs the premultiply loops and every swscale call, stores
//! the formats and converts the output format to the texture format here.
//! The shims take no lock, call nothing in libobs and stay hidden
//! (`rust-hidden.d`).
//!
//! Outside the C's contract, where the C reads or writes out of bounds,
//! [`gs_ffmpeg_image_copy_rows`] copies nothing: a null pointer, or a size
//! that overflows (`graphics-ffmpeg.c` passes the decoded frame's own plane
//! and line size and an output of `cx * cy * 4` bytes).

use core::ffi::c_int;
use core::mem::MaybeUninit;

use crate::image_route::{self as route_core, TEXEL_BYTES};

/// `struct gs_ffmpeg_image_plan`.
#[repr(C)]
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct gs_ffmpeg_image_plan {
    /// `GS_FFMPEG_IMAGE_ROUTE_*`.
    pub route: c_int,
    /// `GS_FFMPEG_IMAGE_PREMULTIPLY_*`.
    pub premultiply: c_int,
    /// The pixel format swscale is given.
    pub sws_format: c_int,
    /// Whether swscale is told the input is full range.
    pub full_range: bool,
    /// The pixel format of the output.
    pub out_format: c_int,
}

/// `struct gs_ffmpeg_image_plan gs_ffmpeg_image_plan(int pix_fmt, int
/// alpha_mode)`.
#[unsafe(no_mangle)]
pub extern "C" fn gs_ffmpeg_image_plan(pix_fmt: c_int, alpha_mode: c_int) -> gs_ffmpeg_image_plan {
    let plan = route_core::plan(pix_fmt, alpha_mode);
    gs_ffmpeg_image_plan {
        route: plan.route.code(),
        premultiply: plan.premultiply.code(),
        sws_format: plan.sws_format,
        full_range: plan.full_range,
        out_format: plan.out_format,
    }
}

/// `int gs_ffmpeg_image_color_format(int pix_fmt)`.
#[unsafe(no_mangle)]
pub extern "C" fn gs_ffmpeg_image_color_format(pix_fmt: c_int) -> c_int {
    route_core::color_format(pix_fmt)
}

/// `void gs_ffmpeg_image_copy_rows(uint8_t *dst, const uint8_t *src, size_t
/// cx, size_t cy, size_t src_linesize)`.
///
/// # Safety
///
/// `dst` points to `cx * cy * 4` writable bytes (they need not be
/// initialized) and `src` to the bytes the copy reads, `(cy - 1) *
/// src_linesize` plus the last row's bytes ([`route_core::copy_src_len`]),
/// not overlapping, or the call copies nothing (a null pointer, or a size
/// that overflows).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_ffmpeg_image_copy_rows(
    dst: *mut u8,
    src: *const u8,
    cx: usize,
    cy: usize,
    src_linesize: usize,
) {
    let Some(dst_len) = cx.checked_mul(cy).and_then(|n| n.checked_mul(TEXEL_BYTES)) else {
        return;
    };
    let Some(src_len) = route_core::copy_src_len(cx, cy, src_linesize) else {
        return;
    };
    if dst.is_null()
        || src.is_null()
        || dst_len == 0
        || src_len == 0
        || isize::try_from(dst_len).is_err()
        || isize::try_from(src_len).is_err()
    {
        return;
    }
    // SAFETY: the caller's contract: `dst_len` writable bytes at `dst` and
    // `src_len` readable bytes at `src`, not overlapping, live for the call.
    // `MaybeUninit<u8>` has `u8`'s layout, and the core only writes it.
    let (dst, src) = unsafe {
        (
            core::slice::from_raw_parts_mut(dst.cast::<MaybeUninit<u8>>(), dst_len),
            core::slice::from_raw_parts(src, src_len),
        )
    };
    route_core::copy_rows(dst, src, cx, cy, src_linesize);
}
