//! `oracle_*` build of `libobs/graphics/graphics-ffmpeg-orient.c.h`
//! (oracle/image_orient.c): the image orientation and size check of
//! `graphics-ffmpeg.c`, renamed.

use core::ffi::c_int;

unsafe extern "C" {
    pub fn oracle_gs_ffmpeg_image_matrix_orientation(a: i32, b: i32, c: i32, d: i32) -> c_int;

    pub fn oracle_gs_ffmpeg_image_orient_needed(orient: c_int) -> bool;

    pub fn oracle_gs_ffmpeg_image_orient_swaps(orient: c_int) -> bool;

    /// Reads and writes `cx * cy * 4` bytes: never call it with less.
    pub fn oracle_gs_ffmpeg_image_orient_copy(
        dst: *mut u8,
        src: *const u8,
        cx: usize,
        cy: usize,
        orient: c_int,
    );

    pub fn oracle_gs_ffmpeg_image_too_large(max_size: u32, cx: c_int, cy: c_int) -> bool;
}
