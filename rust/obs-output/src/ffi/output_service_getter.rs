//! Field-address conversion only; the private output layout stays in C.
use crate::output_service_getter;
use core::ffi::{c_char, c_int, c_void};

#[repr(C)]
pub struct Fields {
    pub service: *const *mut c_void,
}

unsafe extern "C" {
    fn obs_output_service_getter_fields(output: *const c_void) -> Fields;
    fn blog(level: c_int, format: *const c_char, ...);
}

/// # Safety
/// A non-null output must remain live and readable under the original C contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_get_service(output: *const c_void) -> *mut c_void {
    let service = if output.is_null() {
        None
    } else {
        // SAFETY: NULL was rejected before taking a C member address; the caller
        // keeps this output and its service field readable throughout the call.
        Some(unsafe { *obs_output_service_getter_fields(output).service })
    };
    output_service_getter::service(service, |name| {
        // SAFETY: the format and name are static, terminated strings.
        unsafe { blog(400, c"%s: Null 'output' parameter".as_ptr(), name.as_ptr()) }
    })
}
