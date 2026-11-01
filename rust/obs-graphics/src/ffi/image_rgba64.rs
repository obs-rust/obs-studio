//! C ABI for `libobs/graphics/graphics-ffmpeg-rgba64.c.h`, the 16-bit RGBA
//! rows of `graphics-ffmpeg.c`, declared in
//! `libobs/graphics/graphics-ffmpeg-rgba64.h` without `EXPORT`.
//!
//! `graphics-ffmpeg.c` (still C) picks the branch from the frame's format,
//! allocates the `cx * cy * 4`-byte output with `bmalloc()` (not zeroed)
//! and passes it here with the frame's first plane and line size, then
//! stores the output format. The shim takes no lock, calls nothing in
//! libobs and stays hidden (`rust-hidden.txt`).
//!
//! Outside the C's contract, where the C reads or writes out of bounds, the
//! shim writes nothing: a null pointer, a negative `cx`, or a size that
//! overflows (`graphics-ffmpeg.c` passes the decoded frame's own plane and
//! an output of `cx * cy * 4` bytes for a decoded size).

use core::ffi::c_int;
use core::mem::MaybeUninit;

use crate::image_rgba64::{self as rgba64_core, DST_TEXEL_BYTES};

/// `void gs_ffmpeg_image_rgba64be_rows(uint8_t *dst, const uint8_t *src,
/// int cx, int cy, size_t src_linesize, int alpha_mode)`.
///
/// # Safety
///
/// `dst` points to `cx * cy * 4` writable bytes (they need not be
/// initialized) and `src` to the bytes the rows read, `(cy - 1) *
/// src_linesize` plus the last row's texels ([`rgba64_core::src_len`]),
/// not overlapping, or the call writes nothing (a null pointer, a negative
/// `cx`, or a size that overflows).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_ffmpeg_image_rgba64be_rows(
    dst: *mut u8,
    src: *const u8,
    cx: c_int,
    cy: c_int,
    src_linesize: usize,
    alpha_mode: c_int,
) {
    let (Ok(width), rows) = (usize::try_from(cx), usize::try_from(cy).unwrap_or(0)) else {
        return;
    };
    let Some(dst_len) = width
        .checked_mul(rows)
        .and_then(|n| n.checked_mul(DST_TEXEL_BYTES))
    else {
        return;
    };
    let Some(src_len) = rgba64_core::src_len(cx, cy, src_linesize) else {
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
    rgba64_core::rgba64be_rows(dst, src, cx, cy, src_linesize, alpha_mode);
}
