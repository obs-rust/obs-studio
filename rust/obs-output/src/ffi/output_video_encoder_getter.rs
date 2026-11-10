//! Opaque-pointer conversion; validation and ordering live in the safe core.
use crate::output_video_encoder_getter::video_encoder;
use core::ffi::{c_char, c_int, c_void};

unsafe extern "C" {
    fn obs_output_video_encoder_getter_at(output: *const c_void, idx: usize) -> *mut c_void;
    fn blog(level: c_int, format: *const c_char, ...);
}

/// # Safety
/// A non-NULL output and its borrowed encoders must remain live and readable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_get_video_encoder2(
    output: *const c_void,
    idx: usize,
) -> *mut c_void {
    video_encoder(
        !output.is_null(),
        idx,
        |idx| {
            // SAFETY: the core rejects NULL and out-of-range indices before
            // invoking this closure; the caller keeps the output readable.
            unsafe { obs_output_video_encoder_getter_at(output, idx) }
        },
        |name| {
            // SAFETY: both strings are static and NUL terminated.
            unsafe { blog(400, c"%s: Null 'output' parameter".as_ptr(), name.as_ptr()) }
        },
    )
}
