//! Read only the existing identifier accessor, after validating NULL.
use core::ffi::c_void;

/// Whether the source's identifier is exactly `scene`.
///
/// # Safety
/// A non-NULL source must be readable with a live NUL-terminated identifier.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_is_scene(source: *const c_void) -> bool {
    // SAFETY: the caller supplies the original C source lifetime contract.
    crate::kind::is_scene(unsafe { super::from_source::identifier(source) })
}

/// Whether the source's identifier is exactly `group`.
///
/// # Safety
/// A non-NULL source must be readable with a live NUL-terminated identifier.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_is_group(source: *const c_void) -> bool {
    // SAFETY: the caller supplies the original C source lifetime contract.
    crate::kind::is_group(unsafe { super::from_source::identifier(source) })
}
