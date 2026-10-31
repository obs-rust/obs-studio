//! `oracle_*` build of `libobs/graphics/graphics-ffmpeg-rgba64.c.h`
//! (oracle/image_rgba64.c): the 16-bit RGBA rows of `graphics-ffmpeg.c`,
//! renamed.

use core::ffi::c_int;

unsafe extern "C" {
    /// Reads `(cy - 1) * src_linesize` plus the last row's texels and
    /// writes up to `cx * cy * 4` bytes: never call it with less.
    pub fn oracle_gs_ffmpeg_image_rgba64be_rows(
        dst: *mut u8,
        src: *const u8,
        cx: c_int,
        cy: c_int,
        src_linesize: usize,
        alpha_mode: c_int,
    );
}
