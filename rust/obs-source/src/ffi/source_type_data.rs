//! Borrow the type-data member address without mirroring the private source layout.
use core::ffi::{CStr, c_char, c_int, c_void};

#[repr(C)]
pub struct Fields {
    pub data: *const *mut c_void,
}

unsafe extern "C" {
    fn obs_source_type_data_fields(source: *const c_void) -> Fields;
    fn blog(level: c_int, format: *const c_char, ...);
}

fn log(name: &'static CStr) {
    // SAFETY: both format and function name are static NUL-terminated strings.
    unsafe { blog(400, c"%s: Null 'source' parameter".as_ptr(), name.as_ptr()) }
}

/// # Safety
/// A non-null source must remain live and readable for the duration of the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_get_type_data(source: *mut c_void) -> *mut c_void {
    let data = if source.is_null() {
        None
    } else {
        // SAFETY: caller supplies a live source; NULL was rejected before taking a member address.
        Some(unsafe { *obs_source_type_data_fields(source).data })
    };
    crate::type_data::get(data, log)
}
