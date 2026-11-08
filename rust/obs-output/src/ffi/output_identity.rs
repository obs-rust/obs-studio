//! Field-address conversion; the private output layout remains in C.
use crate::output_identity;
use core::ffi::{CStr, c_char, c_int, c_void};

#[repr(C)]
pub struct Fields {
    pub id: *const *const c_char,
    pub type_data: *const *mut c_void,
}

unsafe extern "C" {
    fn obs_output_identity_fields(output: *const c_void) -> Fields;
    fn blog(level: c_int, format: *const c_char, ...);
}

fn log(name: &'static CStr) {
    // SAFETY: literal format and a static terminated name match the argument.
    unsafe { blog(400, c"%s: Null 'output' parameter".as_ptr(), name.as_ptr()) }
}

/// # Safety
/// A non-null output must remain live with readable metadata for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_get_id(output: *const c_void) -> *const c_char {
    let value = if output.is_null() {
        None
    } else {
        // SAFETY: null is rejected before the C member-address accessor; the
        // caller retains the output and the borrowed ID field throughout.
        Some(unsafe { *obs_output_identity_fields(output).id })
    };
    output_identity::id(value, log)
}

/// # Safety
/// A non-null output must remain live with readable metadata for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_get_type_data(output: *mut c_void) -> *mut c_void {
    let value = if output.is_null() {
        None
    } else {
        // SAFETY: null is rejected before the C member-address accessor; only
        // the stored pointer is copied, without accessing its pointee.
        Some(unsafe { *obs_output_identity_fields(output).type_data })
    };
    output_identity::type_data(value, log)
}
