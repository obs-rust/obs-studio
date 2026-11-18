//! C boundary for `obs_output_video()`; the output and encoder layouts stay
//! in C. The flags and raw video come through the existing size-getter
//! adapters (`obs-output-size-getters-fields.c.h`), the encoded branch through
//! the already Rust-backed `obs_output_get_video_encoder()` and
//! `obs_encoder_video()`.
use crate::ffi::output_encoder_getters::obs_output_get_video_encoder;
use crate::output_video_getter::video;
use core::ffi::{c_char, c_int, c_void};

unsafe extern "C" {
    fn obs_output_size_getters_flags(output: *const c_void) -> u32;
    fn obs_output_size_getters_video(output: *const c_void) -> *mut c_void;
    fn obs_encoder_video(encoder: *const c_void) -> *mut c_void;
    fn blog(level: c_int, format: *const c_char, ...);
}

/// Borrowed raw or encoder video; NULL for a NULL output or no video.
///
/// # Safety
/// A non-NULL output and its borrowed encoders must remain live and readable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_video(output: *const c_void) -> *mut c_void {
    video(
        !output.is_null(),
        || {
            // SAFETY: the core rejects NULL before reading flags; the caller
            // keeps the output live.
            unsafe { obs_output_size_getters_flags(output) }
        },
        || {
            // SAFETY: as above; the video is borrowed, not referenced.
            unsafe { obs_output_size_getters_video(output) }
        },
        || {
            // SAFETY: as above. Both getters keep their own NULL handling and
            // diagnostics; a missing encoder reaches obs_encoder_video as NULL.
            unsafe { obs_encoder_video(obs_output_get_video_encoder(output)) }
        },
        |name| {
            // SAFETY: both strings are static and NUL terminated; %s takes the name.
            unsafe { blog(400, c"%s: Null 'output' parameter".as_ptr(), name.as_ptr()) }
        },
    )
}
