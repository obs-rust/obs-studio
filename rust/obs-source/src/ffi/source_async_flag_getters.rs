//! C ABI over borrowed fields, without mirroring the private source layout.

use crate::async_flag_getters;
use core::ffi::{CStr, c_char, c_int, c_void};

#[repr(C)]
pub struct AsyncFlagFields {
    pub unbuffered: *const bool,
    pub decoupled: *const bool,
}

unsafe extern "C" {
    fn obs_source_async_flag_fields(source: *const c_void) -> AsyncFlagFields;
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
/// A non-null source must be live, with its async unbuffered flag readable under the same
/// synchronization required by the original C getter.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_async_unbuffered(source: *const c_void) -> bool {
    // SAFETY: the adapter only returns borrowed addresses, or NULL fields.
    let fields = unsafe { obs_source_async_flag_fields(source) };
    // SAFETY: only the requested field is read; the source stays live.
    async_flag_getters::unbuffered(unsafe { fields.unbuffered.as_ref() }.copied(), null_log)
}

/// # Safety
/// A non-null source must be live, with its async decoupled flag readable under the
/// same synchronization required by the original C getter.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_async_decoupled(source: *const c_void) -> bool {
    // SAFETY: the adapter only returns borrowed addresses, or NULL fields.
    let fields = unsafe { obs_source_async_flag_fields(source) };
    // SAFETY: only the requested field is read; the source stays live.
    async_flag_getters::decoupled(unsafe { fields.decoupled.as_ref() }.copied(), null_log)
}
