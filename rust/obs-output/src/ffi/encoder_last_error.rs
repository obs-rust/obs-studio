//! Borrow one field address from the real private encoder; never mirror its layout.
use crate::encoder_last_error;
use core::ffi::{c_char, c_int, c_void};

#[repr(C)]
pub struct LastErrorFields {
    pub message: *const *const c_char,
}

unsafe extern "C" {
    fn obs_encoder_last_error_fields(encoder: *mut c_void) -> LastErrorFields;
    fn blog(level: c_int, format: *const c_char, ...);
}

/// # Safety
/// A non-null encoder and its message field must be readable during the call.
/// The returned pointer retains the original C getter's borrowed lifetime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_get_last_error(encoder: *mut c_void) -> *const c_char {
    // SAFETY: the adapter only takes a field address, or returns NULL for no encoder.
    let fields = unsafe { obs_encoder_last_error_fields(encoder) };
    // SAFETY: the caller keeps the encoder's field readable. The string is not accessed.
    encoder_last_error::message(unsafe { fields.message.as_ref() }.copied(), || {
        // SAFETY: a static NUL-terminated format with no conversion arguments.
        unsafe {
            blog(
                400,
                c"obs_encoder_get_last_error: Null 'encoder' parameter".as_ptr(),
            )
        };
    })
}
