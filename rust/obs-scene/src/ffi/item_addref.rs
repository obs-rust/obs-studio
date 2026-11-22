//! Opaque scene items: C retains the reference counter's layout, width and
//! atomic ordering.
use crate::item_addref::{self, ItemRef};
use core::ffi::c_void;
use core::ptr::NonNull;

unsafe extern "C" {
    fn obs_sceneitem_addref_increment(item: *mut c_void);
}

struct Item(NonNull<c_void>);
impl ItemRef for Item {
    fn increment(&self) {
        // SAFETY: constructed only after NULL validation; the caller keeps the
        // item live. C performs the original os_atomic_inc_long on `ref`.
        unsafe { obs_sceneitem_addref_increment(self.0.as_ptr()) };
    }
}

/// Add one reference to a scene item; NULL is a no-op.
///
/// # Safety
/// A non-null item must remain live for the call, as in the original C API.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_addref(item: *mut c_void) {
    item_addref::addref(NonNull::new(item).map(Item).as_ref());
}
