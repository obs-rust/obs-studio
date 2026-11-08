//! Public C ABI for built-in source-type queries.
use crate::source_type_queries;
use core::ffi::{CStr, c_char};

/// Query the built-in group identifier.
///
/// # Safety
/// A non-NULL `id` must point to a readable NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_type_is_group(id: *const c_char) -> bool {
    // SAFETY: the caller guarantees a valid C string when non-NULL.
    source_type_queries::is_group(unsafe { identifier(id) })
}

/// Query the built-in scene identifier.
///
/// # Safety
/// A non-NULL `id` must point to a readable NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_type_is_scene(id: *const c_char) -> bool {
    // SAFETY: the caller guarantees a valid C string when non-NULL.
    source_type_queries::is_scene(unsafe { identifier(id) })
}

unsafe fn identifier<'a>(id: *const c_char) -> Option<&'a [u8]> {
    if id.is_null() {
        None
    } else {
        // SAFETY: caller guarantees a readable NUL-terminated string.
        Some(unsafe { CStr::from_ptr(id) }.to_bytes())
    }
}
