//! Thin ABI bridge over private member addresses; no source layout is mirrored.
use core::ffi::{CStr, c_char, c_int, c_uint, c_void};

unsafe extern "C" {
    fn obs_source_type_id_type_field(source: *const c_void) -> *const c_uint;
    fn obs_source_type_id_id_field(source: *const c_void) -> *const *const c_char;
    fn blog(level: c_int, format: *const c_char, ...);
}

fn log(name: &'static CStr) {
    // SAFETY: format and getter name are static NUL-terminated strings.
    unsafe { blog(400, c"%s: Null 'source' parameter".as_ptr(), name.as_ptr()) }
}

/// # Safety
/// A non-null source must remain live and readable throughout the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_get_type(source: *const c_void) -> c_uint {
    let value = if source.is_null() {
        None
    } else {
        // SAFETY: the live source is checked before forming a member address.
        Some(unsafe { *obs_source_type_id_type_field(source) })
    };
    crate::type_id_getters::get_type(value, log)
}

/// # Safety
/// A non-null source must remain live and readable; the returned ID is borrowed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_get_id(source: *const c_void) -> *const c_char {
    let value = if source.is_null() {
        None
    } else {
        // SAFETY: the live source is checked before forming a member address.
        Some(unsafe { *obs_source_type_id_id_field(source) })
    };
    crate::type_id_getters::get_id(value, log)
}
