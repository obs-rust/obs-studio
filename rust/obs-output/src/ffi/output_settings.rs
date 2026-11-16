//! Convert the opaque handle; the private output layout remains in C.
use crate::output_settings;
use core::ffi::{c_char, c_int, c_void};

unsafe extern "C" {
    fn obs_output_settings_fields(output: *const c_void) -> *const *mut c_void;
    fn obs_data_addref(data: *mut c_void);
    fn blog(level: c_int, format: *const c_char, ...);
}

/// # Safety
/// A non-null output and its settings must be live under the original C contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_get_settings(output: *const c_void) -> *mut c_void {
    let value = if output.is_null() {
        None
    } else {
        // SAFETY: NULL was rejected before taking a member address; the caller
        // keeps the output's settings field readable throughout this call.
        Some(unsafe { *obs_output_settings_fields(output) })
    };
    output_settings::settings(
        value,
        |data| {
            // SAFETY: the stored settings handle is live or NULL, as required by C.
            unsafe { obs_data_addref(data) }
        },
        |name| {
            // SAFETY: the format and name are static, terminated strings.
            unsafe { blog(400, c"%s: Null 'output' parameter".as_ptr(), name.as_ptr()) }
        },
    )
}
