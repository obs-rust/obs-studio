//! `oracle_*` build of `libobs/graphics/graphics-ffmpeg-host-wic-checks.c.h`
//! (oracle/image_wic_checks.c): the file-name check and the pixel-format
//! pick of the image loader's Windows host, renamed.

use core::ffi::{c_char, c_int};

unsafe extern "C" {
    /// Reads from `file + len - 4`: never call it with `len` below 4, or
    /// with a `file` that has no NUL at `len`.
    pub fn oracle_gs_image_host_fallback_refuses(file: *const c_char, len: usize) -> bool;

    pub fn oracle_gs_image_wic_kind(scrgb: bool, pq10: bool) -> c_int;
}
