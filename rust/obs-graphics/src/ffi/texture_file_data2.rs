//! C ABI for `libobs/graphics/texture-file-data2.c.h`.
//!
//! `gs_create_texture_file_data2` forwards to `gs_create_texture_file_data3`,
//! which stays C (`graphics-ffmpeg.c.h`, behind FFmpeg): libobs links it,
//! and cargo tests get the oracle crate's recording stand-in
//! (`oracle/texture_render_device.c`). [`create_texture_file_data2_with`]
//! takes data3 as a function pointer so the differential tests can hand
//! both the Rust and the original C a thread-local recorder; the export
//! passes the real symbol.

use core::ffi::c_char;

use crate::image_file::ImageAlphaMode;
use crate::texture_file_data2::create_texture_file_data2;
use crate::texture_render::{ColorFormat, ColorSpace};

/// `gs_create_texture_file_data3`'s C signature.
pub type CreateTextureFileData3 = unsafe extern "C" fn(
    file: *const c_char,
    alpha_mode: ImageAlphaMode,
    format: *mut ColorFormat,
    cx_out: *mut u32,
    cy_out: *mut u32,
    space: *mut ColorSpace,
) -> *mut u8;

unsafe extern "C" {
    fn gs_create_texture_file_data3(
        file: *const c_char,
        alpha_mode: ImageAlphaMode,
        format: *mut ColorFormat,
        cx_out: *mut u32,
        cy_out: *mut u32,
        space: *mut ColorSpace,
    ) -> *mut u8;
}

/// `gs_create_texture_file_data2` over a given data3.
///
/// # Safety
/// The arguments meet `data3`'s own contract, the colour-space output
/// excepted, which this function supplies.
pub unsafe fn create_texture_file_data2_with(
    data3: CreateTextureFileData3,
    file: *const c_char,
    alpha_mode: ImageAlphaMode,
    format: *mut ColorFormat,
    cx_out: *mut u32,
    cy_out: *mut u32,
) -> *mut u8 {
    create_texture_file_data2(|space| {
        // SAFETY: the caller's arguments pass through unchanged; `space` is
        // the core's private slot, live and writable for this call.
        unsafe { data3(file, alpha_mode, format, cx_out, cy_out, space) }
    })
}

/// Decode an image file to pixels, discarding its colour space.
///
/// # Safety
/// As `gs_create_texture_file_data3`: `file` is a NUL-terminated path and
/// `format`, `cx_out` and `cy_out` are writable. A non-null result is a
/// `bmalloc` block the caller frees with `bfree`.
pub unsafe extern "C" fn gs_create_texture_file_data2(
    file: *const c_char,
    alpha_mode: ImageAlphaMode,
    format: *mut ColorFormat,
    cx_out: *mut u32,
    cy_out: *mut u32,
) -> *mut u8 {
    // SAFETY: the caller's contract is data3's.
    unsafe {
        create_texture_file_data2_with(
            gs_create_texture_file_data3,
            file,
            alpha_mode,
            format,
            cx_out,
            cy_out,
        )
    }
}
