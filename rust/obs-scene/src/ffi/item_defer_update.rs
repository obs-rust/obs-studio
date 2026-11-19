//! Opaque scene items: C retains the layout, atomic width and ordering.
use super::item_bounds_getters::SceneItemPrefix;
use crate::item_defer_update::{self, TransformDeferral};
use core::ffi::{CStr, c_char, c_int, c_long, c_void};
use core::ptr::NonNull;

unsafe extern "C" {
    fn obs_sceneitem_defer_update_increment(item: *mut c_void) -> c_long;
    fn obs_sceneitem_defer_update_decrement(item: *mut c_void) -> c_long;
    fn obs_sceneitem_bounds_update_transform(item: *mut SceneItemPrefix);
    fn blog(level: c_int, format: *const c_char, ...);
}

struct Item(NonNull<c_void>);
impl TransformDeferral for Item {
    fn increment(&self) {
        // SAFETY: validated live item; C performs the original atomic RMW.
        unsafe { obs_sceneitem_defer_update_increment(self.0.as_ptr()) };
    }
    fn decrement(&self) -> isize {
        // SAFETY: same live item. C long fits isize on all supported targets.
        unsafe { obs_sceneitem_defer_update_decrement(self.0.as_ptr()) as isize }
    }
    fn update_transform(&self) {
        // SAFETY: same live item; reuse the existing scene dispatch adapter.
        unsafe { obs_sceneitem_bounds_update_transform(self.0.as_ptr().cast()) };
    }
}

fn validated(item: *mut c_void, function: &CStr) -> Option<Item> {
    let item = NonNull::new(item);
    if item.is_none() {
        // SAFETY: static NUL-terminated format and string arguments.
        unsafe {
            blog(
                400,
                c"%s: Null '%s' parameter".as_ptr(),
                function.as_ptr(),
                c"item".as_ptr(),
            );
        }
    }
    item.map(Item)
}

/// # Safety
/// A non-null item must remain live for the call, as in the original C API.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_defer_update_begin(item: *mut c_void) {
    if let Some(item) = validated(item, c"obs_sceneitem_defer_update_begin") {
        item_defer_update::begin(&item);
    }
}

/// # Safety
/// A non-null item must remain live for the call, as in the original C API.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_defer_update_end(item: *mut c_void) {
    if let Some(item) = validated(item, c"obs_sceneitem_defer_update_end") {
        item_defer_update::end(&item);
    }
}
