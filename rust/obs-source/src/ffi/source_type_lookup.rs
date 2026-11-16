//! The ported `get_source_info` as the other shims use it: a borrowed,
//! read-only registry entry. ID policy belongs to `source_info_lookup`.
use core::ffi::{c_char, c_void};

/// # Safety
/// Same contract as [`super::source_info_lookup::get_source_info`]: OBS is
/// initialized and `id` is NULL or a readable NUL-terminated string.
pub unsafe fn get_source_info(id: *const c_char) -> *const c_void {
    // SAFETY: the caller upholds the lookup's contract.
    unsafe { super::source_info_lookup::get_source_info(id) }.cast_const()
}
