//! Alignment setter over the existing layout-checked item prefix and C locks.
use super::item_bounds_getters::SceneItemPrefix;
use crate::item_alignment_setter::{self, Item};
use core::{ffi::c_void, ptr};

unsafe extern "C" {
    fn obs_sceneitem_fields_lock(item: *const SceneItemPrefix) -> *mut c_void;
    fn obs_sceneitem_fields_unlock(scene: *mut c_void);
    fn obs_sceneitem_bounds_update_transform(item: *mut SceneItemPrefix);
}

struct LiveItem(*mut SceneItemPrefix);

impl Item for LiveItem {
    type Parent = *mut c_void;
    fn lock(&mut self) -> Self::Parent {
        // SAFETY: constructed only after the NULL check; caller keeps item/parent live.
        unsafe { obs_sceneitem_fields_lock(self.0) }
    }
    fn store(&mut self, alignment: u32) {
        // SAFETY: the fields lock is held; write only the measured align field.
        unsafe { ptr::addr_of_mut!((*self.0).align).write(alignment) };
    }
    fn unlock(&mut self, parent: Self::Parent) {
        // SAFETY: release the exact captured parent, including NULL for detached items.
        unsafe { obs_sceneitem_fields_unlock(parent) };
    }
    fn update(&mut self) {
        // SAFETY: live item after unlock; C retains the full transform dispatch.
        unsafe { obs_sceneitem_bounds_update_transform(self.0) };
    }
}

/// Store all alignment bits, then request a transform update.
///
/// # Safety
/// A non-null item and its parent must remain live under the C lifecycle contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_set_alignment(item: *mut SceneItemPrefix, alignment: u32) {
    if item.is_null() {
        return;
    }
    item_alignment_setter::set(Some(&mut LiveItem(item)), alignment);
}
