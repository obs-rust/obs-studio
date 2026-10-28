//! C ABI for `libobs/graphics/graphics-ffmpeg-orient.c.h`, the image
//! orientation and size check of `graphics-ffmpeg.c`, declared in
//! `libobs/graphics/graphics-ffmpeg-orient.h` without `EXPORT`.
//!
//! `graphics-ffmpeg.c` (still C) reads the orientation (the display matrix
//! entries cross by value) and the sizes, allocates the turned image and
//! passes it here with the decoded one, then updates the size, frees the
//! decoded image and logs. The shims take no lock, call nothing in libobs
//! and stay hidden (`rust-hidden.txt`).
//!
//! Intentional differences: the core's (an orientation outside 0 to 8 is
//! the image as stored); and [`gs_ffmpeg_image_orient_copy`] copies nothing
//! through a null pointer or for a size whose `cx * cy * 4` overflows
//! (libobs always passes two buffers of that size).

use core::ffi::c_int;

use crate::image_orient::{self as orient_core, BYTES_PER_PIXEL};

/// `int gs_ffmpeg_image_matrix_orientation(int32_t a, int32_t b, int32_t c,
/// int32_t d)`.
#[unsafe(no_mangle)]
pub extern "C" fn gs_ffmpeg_image_matrix_orientation(a: i32, b: i32, c: i32, d: i32) -> c_int {
    orient_core::matrix_orientation(a, b, c, d)
}

/// `bool gs_ffmpeg_image_orient_needed(int orient)`.
#[unsafe(no_mangle)]
pub extern "C" fn gs_ffmpeg_image_orient_needed(orient: c_int) -> bool {
    orient_core::orient_needed(orient)
}

/// `bool gs_ffmpeg_image_orient_swaps(int orient)`.
#[unsafe(no_mangle)]
pub extern "C" fn gs_ffmpeg_image_orient_swaps(orient: c_int) -> bool {
    orient_core::orient_swaps(orient)
}

/// `void gs_ffmpeg_image_orient_copy(uint8_t *dst, const uint8_t *src,
/// size_t cx, size_t cy, int orient)`.
///
/// # Safety
///
/// `src` points to `cx * cy * 4` readable bytes and `dst` to as many
/// writable bytes that do not overlap them, or the call copies nothing
/// (a null pointer, or a size that overflows).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_ffmpeg_image_orient_copy(
    dst: *mut u8,
    src: *const u8,
    cx: usize,
    cy: usize,
    orient: c_int,
) {
    let Some(len) = cx
        .checked_mul(cy)
        .and_then(|n| n.checked_mul(BYTES_PER_PIXEL))
    else {
        return;
    };
    if dst.is_null() || src.is_null() || len == 0 {
        return;
    }
    // SAFETY: the caller's contract: `len` readable bytes at `src` and `len`
    // writable bytes at `dst`, not overlapping, live for the call.
    let (dst, src) = unsafe {
        (
            core::slice::from_raw_parts_mut(dst, len),
            core::slice::from_raw_parts(src, len),
        )
    };
    orient_core::orient_copy(dst, src, cx, cy, orient);
}

/// `bool gs_ffmpeg_image_too_large(uint32_t max_size, int cx, int cy)`.
#[unsafe(no_mangle)]
pub extern "C" fn gs_ffmpeg_image_too_large(max_size: u32, cx: c_int, cy: c_int) -> bool {
    orient_core::too_large(max_size, cx, cy)
}
