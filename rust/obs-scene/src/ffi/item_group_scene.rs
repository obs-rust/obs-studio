//! Keep private scene, item and source layouts in C.
use core::ffi::c_void;
use core::ptr::NonNull;

unsafe extern "C" {
    fn obs_sceneitem_group_scene_flag(item: *const c_void) -> *const bool;
    fn obs_sceneitem_group_scene_data(item: *const c_void) -> *const *mut c_void;
}

/// # Safety
/// A non-null item must be live and readable. For a group, its source must
/// also be live and readable, as in C. The returned scene is borrowed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_group_get_scene(item: *const c_void) -> *mut c_void {
    let is_group = NonNull::new(item.cast_mut()).map(|item| {
        // SAFETY: the caller keeps the complete item's group flag readable.
        unsafe { *obs_sceneitem_group_scene_flag(item.as_ptr()) }
    });
    crate::item_group_scene::group_scene(is_group, || {
        // SAFETY: the core invokes this only for a present group. The caller
        // keeps its source live, and C supplies the real context.data field.
        unsafe { *obs_sceneitem_group_scene_data(item) }
    })
}
