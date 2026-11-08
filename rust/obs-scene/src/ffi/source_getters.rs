//! Borrow source fields through the real C layout after checking each handle.
use core::ffi::c_void;
use core::ptr::NonNull;

unsafe extern "C" {
    fn obs_scene_source_fields(scene: *const c_void) -> *const *mut c_void;
    fn obs_sceneitem_source_fields(item: *const c_void) -> *const *mut c_void;
}

/// # Safety
/// A non-null scene must be live and readable under the original C contract.
/// The source is borrowed; this call does not acquire a reference.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_scene_get_source(scene: *const c_void) -> *mut c_void {
    let stored = NonNull::new(scene.cast_mut()).map(|scene| {
        // SAFETY: the caller keeps the non-null scene and its source field readable.
        unsafe { *obs_scene_source_fields(scene.as_ptr()) }
    });
    crate::source_getters::source(stored)
}

/// # Safety
/// A non-null item must be live and readable under the original C contract.
/// The source is borrowed; this call does not acquire a reference.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_get_source(item: *const c_void) -> *mut c_void {
    let stored = NonNull::new(item.cast_mut()).map(|item| {
        // SAFETY: the caller keeps the non-null item and its source field readable.
        unsafe { *obs_sceneitem_source_fields(item.as_ptr()) }
    });
    crate::source_getters::source(stored)
}
