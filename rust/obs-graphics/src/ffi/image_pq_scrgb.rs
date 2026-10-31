//! C ABI for `libobs/graphics/graphics-ffmpeg-host-pq.c.h`, the JPEG XR
//! PQ-to-scRGB conversion of the image loader's Windows host, declared in
//! `libobs/graphics/graphics-ffmpeg-host-pq.h` without `EXPORT`.
//!
//! `graphics-ffmpeg-host-windows.c` (still C) makes every WIC call, sizes
//! and allocates both buffers, copies the decoded texels into `src` and
//! passes both here with their sizes. The shim takes no lock, calls nothing
//! in libobs and stays hidden (`rust-hidden.d`). Only Windows calls it.
//! Elsewhere it is built but unused, and hidden all the same.

use crate::image_pq_scrgb::{self as pq_core, DST_TEXEL_BYTES, SRC_TEXEL_BYTES};

/// `void gs_image_pq10_to_scrgb(uint8_t *dst, uint32_t dst_size, const
/// uint8_t *src, uint32_t src_size)`.
///
/// Converts the whole texels both buffers hold: the smaller of
/// `src_size / 4` and `dst_size / 8`. Converts nothing through a null
/// pointer.
///
/// # Safety
///
/// `src` points to `src_size` readable bytes and `dst` to `dst_size`
/// writable bytes that do not overlap them, or one of them is null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_image_pq10_to_scrgb(
    dst: *mut u8,
    dst_size: u32,
    src: *const u8,
    src_size: u32,
) {
    if dst.is_null() || src.is_null() {
        return;
    }
    let (Ok(dst_size), Ok(src_size)) = (usize::try_from(dst_size), usize::try_from(src_size))
    else {
        return;
    };
    let texels = (src_size / SRC_TEXEL_BYTES).min(dst_size / DST_TEXEL_BYTES);
    let (Some(dst_len), Some(src_len)) = (
        texels.checked_mul(DST_TEXEL_BYTES),
        texels.checked_mul(SRC_TEXEL_BYTES),
    ) else {
        return;
    };
    // SAFETY: the caller's contract: `dst_size` writable bytes at `dst` and
    // `src_size` readable bytes at `src`, not overlapping, live for the
    // call; `dst_len` and `src_len` are at most those sizes.
    let (dst, src) = unsafe {
        (
            core::slice::from_raw_parts_mut(dst, dst_len),
            core::slice::from_raw_parts(src, src_len),
        )
    };
    pq_core::pq10_to_scrgb(dst, src);
}
