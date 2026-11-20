//! `obs_canvas_has_valid_video_info()` over the canvas's real `ovi` field,
//! reached through the private adapter of
//! `libobs/obs-canvas-has-valid-video-info-fields.c.h`.
//!
//! Declared in `obs-internal.h` without `EXPORT`, so the shim stays hidden
//! (`rust-hidden.d/obs-core-canvas_valid_video_info.txt`). Each field is read
//! directly from the C-owned `struct obs_video_info`, unlocked, and only
//! once its earlier conjuncts held, as in C. Nothing is written or
//! allocated.
use core::ffi::{c_int, c_void};
use core::ptr::addr_of;

use super::canvas_video_info_getter::VideoInfo;
use crate::canvas_valid_video_info::{VideoInfoFields, has_valid_video_info};

unsafe extern "C" {
    fn obs_canvas_has_valid_video_info_ovi(canvas: *const c_void) -> *const VideoInfo;
}

/// The canvas's `ovi`, read one field at a time.
struct Fields(*const VideoInfo);

impl VideoInfoFields for Fields {
    fn base_width(&mut self) -> u32 {
        // SAFETY: `self.0` is the live canvas's `ovi` (see the shim).
        unsafe { addr_of!((*self.0).base_width).read() }
    }
    fn base_height(&mut self) -> u32 {
        // SAFETY: as in `base_width`.
        unsafe { addr_of!((*self.0).base_height).read() }
    }
    fn output_width(&mut self) -> u32 {
        // SAFETY: as in `base_width`.
        unsafe { addr_of!((*self.0).output_width).read() }
    }
    fn output_height(&mut self) -> u32 {
        // SAFETY: as in `base_width`.
        unsafe { addr_of!((*self.0).output_height).read() }
    }
    fn output_format(&mut self) -> c_int {
        // SAFETY: as in `base_width`; `enum video_format` is an `int`.
        unsafe { addr_of!((*self.0).output_format).read() }
    }
}

/// True when the canvas's base and output sizes are all nonzero and its
/// output format is not `VIDEO_FORMAT_NONE`.
///
/// # Safety
/// As for the C function: `canvas` is a live canvas (NULL is outside the
/// valid domain and faults, as in C), readable for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_canvas_has_valid_video_info(canvas: *const c_void) -> bool {
    // SAFETY: the adapter only forms the address of the caller's canvas's
    // `ovi`; the reads above stay within it.
    has_valid_video_info(&mut Fields(unsafe {
        obs_canvas_has_valid_video_info_ovi(canvas)
    }))
}
