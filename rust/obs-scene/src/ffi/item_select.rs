//! Selection ABI over the existing real-header-verified item prefix.
use super::item_state_getters::{SceneItemPrefix, obs_sceneitem_selected};
use crate::item_select::{self, Item};
use core::{ffi::c_void, ptr::NonNull};

unsafe extern "C" {
    fn obs_sceneitem_select_signal(item: *mut SceneItemPrefix, parent: *mut c_void, selected: bool);
}
struct BorrowedItem(NonNull<SceneItemPrefix>);
impl Item for BorrowedItem {
    fn selected(&self) -> bool {
        // SAFETY: constructed only for a live item; reuse the ported getter.
        unsafe { obs_sceneitem_selected(self.0.as_ptr()) }
    }
    fn has_parent(&self) -> bool {
        // SAFETY: read only the parent field of the caller's live item.
        unsafe {
            !core::ptr::addr_of!((*self.0.as_ptr()).parent)
                .read()
                .is_null()
        }
    }
    fn store(&self, value: bool) {
        // SAFETY: caller supplies writable selected storage. No reference escapes.
        unsafe {
            core::ptr::addr_of_mut!((*self.0.as_ptr()).selected).write(value);
        }
    }
    fn signal(&self, value: bool) {
        // SAFETY: live item with a parent, after the state write. The C boundary
        // preserves fixed calldata and synchronous signals. Do not access the
        // item after callbacks, which may remove it or recursively select it.
        unsafe {
            let parent = core::ptr::addr_of!((*self.0.as_ptr()).parent).read();
            obs_sceneitem_select_signal(self.0.as_ptr(), parent, value);
        }
    }
}
/// Select or deselect an item before synchronously delivering its scene signal.
///
/// # Safety
/// Non-null items must satisfy the original C lifetime and synchronization
/// contract. The item and parent are live until callbacks may remove them.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_select(item: *mut SceneItemPrefix, selected: bool) {
    let item = NonNull::new(item).map(BorrowedItem);
    item_select::select(item.as_ref(), selected);
}
