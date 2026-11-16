//! Borrow the C array only while its original mutex is held.
use crate::filter_index;
use core::ffi::{CStr, c_char, c_int, c_void};

unsafe extern "C" {
    fn obs_source_filter_index_lock(source: *mut c_void, count: *mut usize) -> *const *mut c_void;
    fn obs_source_filter_index_unlock(source: *mut c_void);
    fn blog(level: c_int, format: *const c_char, ...);
}

fn log(parameter: &'static CStr) {
    // SAFETY: all strings are static and NUL terminated; the format is literal.
    unsafe {
        blog(
            400,
            c"obs_source_filter_get_index: Null '%s' parameter".as_ptr(),
            parameter.as_ptr(),
        );
    }
}

/// # Safety
/// Non-null handles must remain live; source must own an initialized filter
/// mutex and a valid pointer array under the original C synchronization contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_filter_get_index(
    source: *mut c_void,
    filter: *mut c_void,
) -> c_int {
    if !filter_index::valid(!source.is_null(), !filter.is_null(), log) {
        return -1;
    }
    let mut count = 0;
    // SAFETY: validation rejected NULL before the adapter touches private fields.
    let items = unsafe { obs_source_filter_index_lock(source, &mut count) };
    let entries = if count == 0 {
        &[][..]
    } else {
        // SAFETY: C holds filter_mutex; this array stays live and immutable until unlock.
        unsafe { core::slice::from_raw_parts(items, count) }
    };
    let result = filter_index::index(entries, &filter);
    // SAFETY: matching unlock for the live source, after the borrowed array's last use.
    unsafe { obs_source_filter_index_unlock(source) };
    result
}
