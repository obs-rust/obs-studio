//! C ABI over borrowed fields, without mirroring the private source layout.

use crate::deinterlace_getters;
use core::ffi::{CStr, c_char, c_int, c_void};

#[repr(C)]
pub struct DeinterlaceFields {
    pub mode: *const u32,
    pub top_first: *const bool,
}

unsafe extern "C" {
    fn obs_source_deinterlace_fields(source: *const c_void) -> DeinterlaceFields;
    fn blog(level: c_int, format: *const c_char, ...);
}

fn null_log(function: &'static CStr, parameter: &'static CStr) {
    // SAFETY: the format and its two string arguments are NUL terminated.
    unsafe {
        blog(
            400,
            c"%s: Null '%s' parameter".as_ptr(),
            function.as_ptr(),
            parameter.as_ptr(),
        );
    }
}

/// # Safety
/// A non-null source must be live, with its mode readable under the same
/// synchronization required by the original C getter.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_get_deinterlace_mode(source: *const c_void) -> u32 {
    // SAFETY: the adapter only returns borrowed addresses, or NULL fields.
    let fields = unsafe { obs_source_deinterlace_fields(source) };
    // SAFETY: only the requested field is read; the source stays live.
    deinterlace_getters::mode(unsafe { fields.mode.as_ref() }.copied(), null_log)
}

/// # Safety
/// A non-null source must be live, with its field order readable under the
/// same synchronization required by the original C getter.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_get_deinterlace_field_order(source: *const c_void) -> u32 {
    // SAFETY: the adapter only returns borrowed addresses, or NULL fields.
    let fields = unsafe { obs_source_deinterlace_fields(source) };
    // SAFETY: only the requested field is read; the source stays live.
    deinterlace_getters::field_order(unsafe { fields.top_first.as_ref() }.copied(), null_log)
}
