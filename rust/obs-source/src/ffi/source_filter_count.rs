//! Convert the private C field address; reject NULL before the adapter.
use crate::filter_count;
use core::ffi::{CStr, c_char, c_int, c_void};

#[repr(C)]
pub struct Fields {
    pub count: *const usize,
}

unsafe extern "C" {
    fn obs_source_filter_count_fields(source: *const c_void) -> Fields;
    fn blog(level: c_int, format: *const c_char, ...);
}

unsafe fn fields(source: *const c_void) -> Option<usize> {
    if source.is_null() {
        return None;
    }
    // SAFETY: the caller keeps the source readable; NULL was rejected before
    // the C adapter takes a member address. size_t has Rust usize's layout.
    unsafe { Some(*obs_source_filter_count_fields(source).count) }
}

fn log(name: &'static CStr) {
    // SAFETY: both strings are static and NUL terminated; the format is literal.
    unsafe { blog(400, c"%s: Null 'source' parameter".as_ptr(), name.as_ptr()) }
}

/// # Safety
/// A non-null source must be live and readable under the original C contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_filter_count(source: *const c_void) -> usize {
    // SAFETY: the caller supplies a live source or NULL; fields rejects NULL.
    filter_count::count(unsafe { fields(source) }, log)
}
