//! Borrow real C field addresses only after checking the opaque handle.
use core::ffi::c_void;
use core::ptr::NonNull;

unsafe extern "C" {
    fn obs_scene_group_query_fields(scene: *const c_void) -> *const bool;
    fn obs_sceneitem_group_query_fields(item: *const c_void) -> *const bool;
}

/// # Safety
/// A non-null scene must be live and readable under the original C contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_scene_is_group(scene: *const c_void) -> bool {
    let value = NonNull::new(scene.cast_mut()).map(|scene| {
        // SAFETY: the handle is non-null and the caller keeps its field readable.
        unsafe { &*obs_scene_group_query_fields(scene.as_ptr()) }
    });
    crate::group_queries::is_group(value)
}

/// # Safety
/// A non-null item must be live and readable under the original C contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_is_group(item: *mut c_void) -> bool {
    let value = NonNull::new(item).map(|item| {
        // SAFETY: the handle is non-null and the caller keeps its field readable.
        unsafe { &*obs_sceneitem_group_query_fields(item.as_ptr()) }
    });
    crate::group_queries::is_group(value)
}
