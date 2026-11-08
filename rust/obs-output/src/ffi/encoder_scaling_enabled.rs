//! Borrowed scaled-axis addresses from the real private encoder layout.
use crate::encoder_scaling_enabled;
use core::ffi::{CStr, c_char, c_int, c_void};

#[repr(C)]
pub struct ScalingFields {
    pub width: *const u32,
    pub height: *const u32,
}

unsafe extern "C" {
    fn obs_encoder_scaling_enabled_fields(encoder: *const c_void) -> ScalingFields;
    fn blog(level: c_int, format: *const c_char, ...);
}

fn null_log(function: &'static CStr, parameter: &'static CStr) {
    // SAFETY: static NUL-terminated strings match the printf format.
    unsafe {
        blog(
            400,
            c"%s: Null '%s' parameter".as_ptr(),
            function.as_ptr(),
            parameter.as_ptr(),
        )
    };
}

/// # Safety
/// A non-null encoder must remain live with readable scaled dimensions for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_scaling_enabled(encoder: *const c_void) -> bool {
    // SAFETY: the adapter only borrows field addresses, or returns null for no encoder.
    let fields = unsafe { obs_encoder_scaling_enabled_fields(encoder) };
    // SAFETY: both field pointers belong to the caller's live encoder, or are null.
    let values = unsafe {
        fields
            .width
            .as_ref()
            .copied()
            .zip(fields.height.as_ref().copied())
    };
    encoder_scaling_enabled::get(values, null_log)
}
