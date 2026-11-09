//! Private C adapters retain the item layout, atomics and actions mutex.
use crate::set_visibility::{Item, reconcile};
use core::ffi::{c_long, c_void};

unsafe extern "C" {
    fn obs_sceneitem_reconcile_lock(item: *mut c_void);
    fn obs_sceneitem_reconcile_unlock(item: *mut c_void);
    fn obs_sceneitem_reconcile_clear(item: *mut c_void);
    fn obs_sceneitem_reconcile_refs(item: *mut c_void) -> c_long;
    fn obs_sceneitem_reconcile_add(item: *mut c_void);
    fn obs_sceneitem_reconcile_remove(item: *mut c_void);
    fn obs_sceneitem_reconcile_normalize(item: *mut c_void, visible: bool);
}

struct LockedItem(*mut c_void);
impl Item for LockedItem {
    fn clear_actions(&mut self) {
        // SAFETY: the shim holds the live item's actions mutex throughout reconciliation.
        unsafe { obs_sceneitem_reconcile_clear(self.0) }
    }
    #[allow(clippy::unnecessary_cast)] // c_long is 32-bit on Windows and 64-bit on Linux.
    fn active_refs(&self) -> i64 {
        // SAFETY: the item is live and locked; the adapter performs the original atomic load.
        unsafe { obs_sceneitem_reconcile_refs(self.0) as i64 }
    }
    fn add_child(&mut self) {
        // SAFETY: original caller guarantees a live item, parent and source.
        unsafe { obs_sceneitem_reconcile_add(self.0) }
    }
    fn remove_child(&mut self) {
        // SAFETY: original caller guarantees a live item, parent and source.
        unsafe { obs_sceneitem_reconcile_remove(self.0) }
    }
    fn normalize(&mut self, visible: bool) {
        // SAFETY: live locked item; adapter performs the three original atomic stores.
        unsafe { obs_sceneitem_reconcile_normalize(self.0, visible) }
    }
}
impl Drop for LockedItem {
    fn drop(&mut self) {
        // SAFETY: this guard owns the actions mutex locked at construction.
        unsafe { obs_sceneitem_reconcile_unlock(self.0) }
    }
}

/// # Safety
/// `item`, its parent and source must be live, as required by the original private C function.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_reconcile_visibility(item: *mut c_void, visible: bool) {
    // SAFETY: the caller supplies a complete live item and obeys the original lock order.
    unsafe { obs_sceneitem_reconcile_lock(item) };
    reconcile(&mut LockedItem(item), visible);
}
