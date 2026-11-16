//! Private-layout access and the original libobs ownership boundary.
use core::ffi::{CStr, c_char, c_int, c_void};

unsafe extern "C" {
    fn obs_source_settings_getter_fields(source: *const c_void) -> *mut c_void;
    fn obs_data_addref(data: *mut c_void);
    fn blog(level: c_int, format: *const c_char, ...);
}

/// # Safety
/// A non-null source must be live and readable, as required by the C API.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_get_settings(source: *const c_void) -> *mut c_void {
    let settings = if source.is_null() {
        None
    } else {
        // SAFETY: NULL is rejected before calling the private field accessor.
        Some(unsafe { obs_source_settings_getter_fields(source) })
    };
    crate::settings_getter::get(
        settings,
        |settings| {
            // SAFETY: the live source owns this settings reference; addref accepts NULL.
            unsafe { obs_data_addref(settings) }
        },
        |name: &'static CStr| {
            // SAFETY: both strings are static and the variadic argument matches %s.
            unsafe { blog(400, c"%s: Null 'source' parameter".as_ptr(), name.as_ptr()) }
        },
    )
}
