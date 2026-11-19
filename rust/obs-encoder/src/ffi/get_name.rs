//! Opaque encoder projection follows the public NULL guard.
use core::ffi::{c_char, c_int, c_void};

unsafe extern "C" {
    fn obs_encoder_get_name_context_name(encoder: *const c_void) -> *const c_char;
    fn blog(level: c_int, format: *const c_char, ...);
}

/// # Safety
/// A non-NULL encoder must be live; the result borrows its current context name.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_get_name(encoder: *const c_void) -> *const c_char {
    crate::get_name::get(
        (!encoder.is_null()).then_some(|| {
            // SAFETY: get invokes the projection only for a non-NULL live encoder.
            unsafe { obs_encoder_get_name_context_name(encoder) }
        }),
        |level, message| {
            // SAFETY: the static diagnostic is NUL-terminated without substitutions.
            unsafe { blog(level, message.as_ptr()) }
        },
    )
    .unwrap_or(core::ptr::null())
}
