//! C ABI for two files of the image loader's Windows host, both declared
//! without `EXPORT`:
//!
//! - `libobs/graphics/graphics-ffmpeg-host-pq.c.h`, the JPEG XR
//!   PQ-to-scRGB conversion (`graphics-ffmpeg-host-pq.h`):
//!   [`gs_image_pq10_to_scrgb`];
//! - `libobs/graphics/graphics-ffmpeg-host-wic-checks.c.h`, the file-name
//!   check and the pixel-format pick (`graphics-ffmpeg-host-wic-checks.h`):
//!   [`gs_image_host_fallback_refuses`] and [`gs_image_wic_kind`].
//!
//! `graphics-ffmpeg-host-windows.c` (still C) makes every WIC call, takes
//! the name's `strlen()` and compares the pixel-format GUIDs, sizes and
//! allocates both buffers, copies the decoded texels into `src` and passes
//! each value here where it made that decision. The shims take no lock,
//! call nothing in libobs and stay hidden (`rust-hidden.d`). Only Windows
//! calls them. Elsewhere they are built but unused, and hidden all the same.
//! The kind crosses as a C `int`.

use core::ffi::{c_char, c_int};

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

/// `bool gs_image_host_fallback_refuses(const char *file, size_t len)`.
///
/// Refuses a null `file`, which the C caller never passes (it takes the
/// name's `strlen()` first).
///
/// # Safety
///
/// `file` is null or points to `len` readable bytes (the caller passes the
/// name's `strlen()`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_image_host_fallback_refuses(file: *const c_char, len: usize) -> bool {
    if file.is_null() {
        return true;
    }
    // SAFETY: the caller's contract: `len` readable bytes at `file`, live
    // for the call.
    let name = unsafe { core::slice::from_raw_parts(file.cast::<u8>(), len) };
    pq_core::fallback_refuses(name)
}

/// `enum gs_image_wic_kind gs_image_wic_kind(bool scrgb, bool pq10)`.
#[unsafe(no_mangle)]
pub extern "C" fn gs_image_wic_kind(scrgb: bool, pq10: bool) -> c_int {
    pq_core::wic_kind(scrgb, pq10) as c_int
}
