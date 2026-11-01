//! `oracle_*` build of `libobs/graphics/graphics-ffmpeg-route.c.h`
//! (oracle/image_route.c): the pixel route of `graphics-ffmpeg.c`, renamed.

use core::ffi::c_int;

/// `struct gs_ffmpeg_image_plan`, as the oracle returns it.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OraclePlan {
    pub route: c_int,
    pub premultiply: c_int,
    pub sws_format: c_int,
    pub full_range: bool,
    pub out_format: c_int,
}

unsafe extern "C" {
    pub fn oracle_gs_ffmpeg_image_plan(pix_fmt: c_int, alpha_mode: c_int) -> OraclePlan;
    pub fn oracle_gs_ffmpeg_image_color_format(pix_fmt: c_int) -> c_int;
    /// Reads `(cy - 1) * src_linesize` plus the last row's bytes and writes
    /// up to `cx * cy * 4` bytes: never call it with less.
    pub fn oracle_gs_ffmpeg_image_copy_rows(
        dst: *mut u8,
        src: *const u8,
        cx: usize,
        cy: usize,
        src_linesize: usize,
    );
    /// Writes the size, alignment and five field offsets of
    /// `struct gs_ffmpeg_image_plan` to `out` (room for 7) and returns 7.
    pub fn oracle_image_route_plan_layout(out: *mut usize) -> usize;
}
