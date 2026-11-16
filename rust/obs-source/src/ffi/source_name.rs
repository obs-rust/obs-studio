//! Opaque source conversion only; C retains its private layout and name ownership.
use core::ffi::{CStr, c_char, c_int, c_void};

unsafe extern "C" {
    fn obs_source_name_field(source: *const c_void) -> *const c_char;
    fn blog(level: c_int, format: *const c_char, ...);
}

fn log(label: &'static CStr) {
    // SAFETY: static terminated format and label, with the original debug level.
    unsafe { blog(400, c"%s: Null 'source' parameter".as_ptr(), label.as_ptr()) }
}

/// # Safety
/// A non-null source must be live and readable. The result is borrowed under
/// the original C contract, until the source is renamed or destroyed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_get_name(source: *const c_void) -> *const c_char {
    crate::source_name::name(
        !source.is_null(),
        // SAFETY: the core invokes this only for non-null, live sources.
        || unsafe { obs_source_name_field(source) },
        log,
    )
}
