//! The source layout and callback ABI stay at the C boundary.
use crate::replace_missing_file::replace;
use core::ffi::{CStr, c_char, c_int, c_void};

pub type Callback = unsafe extern "C" fn(*mut c_void, *const c_char, *mut c_void);

use super::source_missing_files_getter::Fields;

unsafe extern "C" {
    /// The private `context.data` read, shared with the missing-files getter.
    fn obs_source_missing_files_getter_fields(source: *const c_void) -> Fields;
    fn blog(level: c_int, format: *const c_char, ...);
}

/// # Safety
/// A non-null source and its private data must remain live for the call,
/// and `cb` must be callable with that data, as required by the original C
/// API. A NULL `cb` with live private data aborts (the C calls address 0).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_replace_missing_file(
    cb: Option<Callback>,
    source: *mut c_void,
    new_path: *const c_char,
    data: *mut c_void,
) {
    let source = if source.is_null() {
        None
    } else {
        // SAFETY: NULL was rejected before the private C layout accessor.
        let fields = unsafe { obs_source_missing_files_getter_fields(source) };
        Some((!fields.data.is_null()).then_some(fields.data))
    };
    replace(
        source,
        |private| {
            let Some(cb) = cb else {
                // The C calls through the NULL pointer here.
                std::process::abort()
            };
            // SAFETY: the caller supplies a callback for this source's data.
            unsafe { cb(private, new_path, data) }
        },
        |name: &'static CStr| {
            // SAFETY: static strings and the pointer argument match the %s format.
            unsafe { blog(400, c"%s: Null 'source' parameter".as_ptr(), name.as_ptr()) }
        },
    );
}
