//! C ABI over borrowed relationship fields, without mirroring obs_source.
use crate::filter_relationship_getters;
use core::ffi::{CStr, c_char, c_int, c_void};

#[repr(C)]
pub struct FilterRelationshipFields {
    pub parent: *const *mut c_void,
    pub target: *const *mut c_void,
}

unsafe extern "C" {
    fn obs_source_filter_relationship_fields(filter: *const c_void) -> FilterRelationshipFields;
    fn blog(level: c_int, format: *const c_char, ...);
}

fn null_log(function: &'static CStr, parameter: &'static CStr) {
    // SAFETY: the format and both string arguments are NUL terminated.
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
/// A non-null filter must be live, with filter_parent readable under the original
/// C getter's synchronization contract. The returned pointer is borrowed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_filter_get_parent(filter: *const c_void) -> *mut c_void {
    // SAFETY: the adapter returns borrowed addresses or NULL fields only.
    let fields = unsafe { obs_source_filter_relationship_fields(filter) };
    // SAFETY: only the requested field is read, while the filter remains live.
    filter_relationship_getters::parent(unsafe { fields.parent.as_ref() }.copied(), null_log)
}

/// # Safety
/// A non-null filter must be live, with filter_target readable under the original
/// C getter's synchronization contract. The returned pointer is borrowed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_filter_get_target(filter: *const c_void) -> *mut c_void {
    // SAFETY: the adapter returns borrowed addresses or NULL fields only.
    let fields = unsafe { obs_source_filter_relationship_fields(filter) };
    // SAFETY: only the requested field is read, while the filter remains live.
    filter_relationship_getters::target(unsafe { fields.target.as_ref() }.copied(), null_log)
}
