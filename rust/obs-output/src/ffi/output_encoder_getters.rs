//! Opaque-pointer conversion; the decisions and their order live in the
//! safe core.
use crate::ffi::output_video_encoder_getter::obs_output_get_video_encoder2;
use crate::output_encoder_getters::{audio_encoder, video_encoder};
use core::ffi::{c_char, c_int, c_void};

unsafe extern "C" {
    fn obs_output_encoder_getters_video_slots() -> usize;
    fn obs_output_encoder_getters_video_occupied(output: *const c_void, idx: usize) -> bool;
    fn obs_output_encoder_getters_audio_at(output: *const c_void, idx: usize) -> *mut c_void;
    fn blog(level: c_int, format: *const c_char, ...);
}

fn log_null(name: &'static core::ffi::CStr) {
    // SAFETY: both strings are static and NUL terminated; %s takes the name.
    unsafe { blog(400, c"%s: Null 'output' parameter".as_ptr(), name.as_ptr()) }
}

/// # Safety
/// A non-NULL output and its borrowed encoders must remain live and readable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_get_video_encoder(output: *const c_void) -> *mut c_void {
    video_encoder(
        !output.is_null(),
        // SAFETY: returns a constant of the C build; it reads no output.
        unsafe { obs_output_encoder_getters_video_slots() },
        |idx| {
            // SAFETY: the core asks only below the slot count, for a non-NULL
            // output the caller keeps readable.
            unsafe { obs_output_encoder_getters_video_occupied(output, idx) }
        },
        |idx| {
            // SAFETY: as above; the getter checks the output and index again.
            unsafe { obs_output_get_video_encoder2(output, idx) }
        },
        log_null,
    )
}

/// # Safety
/// A non-NULL output and its borrowed encoders must remain live and readable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_get_audio_encoder(
    output: *const c_void,
    idx: usize,
) -> *mut c_void {
    audio_encoder(
        !output.is_null(),
        idx,
        |idx| {
            // SAFETY: the core rejects NULL and out-of-range indices first.
            unsafe { obs_output_encoder_getters_audio_at(output, idx) }
        },
        log_null,
    )
}
