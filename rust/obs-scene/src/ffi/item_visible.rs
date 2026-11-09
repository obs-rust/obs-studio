//! Keep the private item layout and atomic operation in C.
use core::ffi::c_void;
use core::ptr::NonNull;

unsafe extern "C" {
    fn obs_sceneitem_user_visible_load(item: *const c_void) -> bool;
}

/// # Safety
/// A non-null item must remain live and readable for this call, as in C.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_visible(item: *const c_void) -> bool {
    let value = NonNull::new(item.cast_mut()).map(|item| {
        // SAFETY: the caller keeps the complete item live; C atomically reads user_visible.
        unsafe { obs_sceneitem_user_visible_load(item.as_ptr()) }
    });
    crate::item_visible::visible(value)
}
