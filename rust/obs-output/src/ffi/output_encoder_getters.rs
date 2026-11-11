//! Opaque-pointer conversion; validation and ordering live in the safe core.
use crate::output_encoder_getters::{audio_encoder, video_encoder};
use core::ffi::{c_char, c_int, c_void};

unsafe extern "C" {
    fn obs_output_encoder_getters_video_mask(output: *const c_void) -> u32;
    fn obs_output_encoder_getters_audio_at(output: *const c_void, idx: usize) -> *mut c_void;
    fn obs_output_video_encoder_getter_at(output: *const c_void, idx: usize) -> *mut c_void;
    fn blog(level: c_int, format: *const c_char, ...);
}

/// # Safety
/// A non-NULL output and its borrowed encoders must remain live and readable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_get_video_encoder(output: *const c_void) -> *mut c_void {
    video_encoder(
        !output.is_null(),
        // SAFETY: the core evaluates this only after rejecting NULL.
        || unsafe { obs_output_encoder_getters_video_mask(output) },
        |idx| {
            // SAFETY: the core validated the output and picked an occupied slot.
            unsafe { obs_output_video_encoder_getter_at(output, idx) }
        },
        |name| {
            // SAFETY: both strings are static and NUL terminated.
            unsafe { blog(400, c"%s: Null 'output' parameter".as_ptr(), name.as_ptr()) }
        },
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
        |slot| {
            // SAFETY: the core validated the output and bounded the index.
            unsafe { obs_output_encoder_getters_audio_at(output, slot) }
        },
        |name| {
            // SAFETY: both strings are static and NUL terminated.
            unsafe { blog(400, c"%s: Null 'output' parameter".as_ptr(), name.as_ptr()) }
        },
    )
}
