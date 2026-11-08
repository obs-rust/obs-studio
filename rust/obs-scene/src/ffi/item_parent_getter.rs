//! Borrow the parent field through C's private layout after checking the item.
use core::ffi::c_void;
use core::ptr::NonNull;

unsafe extern "C" {
    fn obs_sceneitem_parent_fields(item: *const c_void) -> *const *mut c_void;
}

/// # Safety
/// A non-null item must be live and readable under the original C contract.
/// The returned scene is borrowed; this call does not acquire a reference.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_get_scene(item: *const c_void) -> *mut c_void {
    let value = NonNull::new(item.cast_mut()).map(|item| {
        // SAFETY: the caller keeps the non-null item and its parent field readable.
        unsafe { *obs_sceneitem_parent_fields(item.as_ptr()) }
    });
    crate::item_parent_getter::parent(value)
}
