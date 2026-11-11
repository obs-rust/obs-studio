//! Private source fields stay in C. Returned context pointers remain borrowed.
use core::ffi::{CStr, c_char, c_void};
use core::ptr::NonNull;

unsafe extern "C" {
    fn obs_scene_from_source_identifier(source: *const c_void) -> *const c_char;
    fn obs_scene_from_source_context(source: *const c_void) -> *mut c_void;
}

unsafe fn convert(source: *const c_void, expected: &[u8]) -> *mut c_void {
    let id = NonNull::new(source.cast_mut()).map(|source| {
        // SAFETY: callers supply a live source with its valid NUL-terminated
        // registered identifier. The private adapter is reached only after NULL validation.
        unsafe { CStr::from_ptr(obs_scene_from_source_identifier(source.as_ptr())) }.to_bytes()
    });
    crate::from_source::from_source(id, expected, || {
        // SAFETY: the core reads context only for a present source with the
        // matching ID. Its context remains live under the original C contract.
        unsafe { obs_scene_from_source_context(source) }
    })
}

/// Borrow the scene context for the registered scene identifier.
///
/// # Safety
/// A non-NULL source must be live and readable with a valid identifier. The
/// returned pointer has the source's lifetime and acquires no reference.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_scene_from_source(source: *const c_void) -> *mut c_void {
    // SAFETY: the caller supplies the original C source lifetime contract.
    unsafe { convert(source, b"scene") }
}

/// Borrow the scene context for the registered group identifier.
///
/// # Safety
/// A non-NULL source must be live and readable with a valid identifier. The
/// returned pointer has the source's lifetime and acquires no reference.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_group_from_source(source: *const c_void) -> *mut c_void {
    // SAFETY: the caller supplies the original C source lifetime contract.
    unsafe { convert(source, b"group") }
}
