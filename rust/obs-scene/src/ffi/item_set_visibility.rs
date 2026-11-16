//! Opaque-item bridge for immediate visibility reconciliation. No C layout is mirrored.
use core::ffi::{c_long, c_void};
use core::ptr::NonNull;

use crate::item_set_visibility::{self, VisibilityItem};

unsafe extern "C" {
    fn obs_sceneitem_visibility_lock(item: *mut c_void);
    fn obs_sceneitem_visibility_unlock(item: *mut c_void);
    fn obs_sceneitem_visibility_clear(item: *mut c_void);
    fn obs_sceneitem_visibility_refs(item: *mut c_void) -> c_long;
    fn obs_sceneitem_visibility_parent_source(item: *mut c_void) -> *mut c_void;
    fn obs_sceneitem_visibility_source(item: *mut c_void) -> *mut c_void;
    fn obs_source_add_active_child(parent: *mut c_void, child: *mut c_void) -> bool;
    fn obs_source_remove_active_child(parent: *mut c_void, child: *mut c_void);
    fn obs_sceneitem_visibility_set_refs(item: *mut c_void, visible: bool);
    fn obs_sceneitem_visibility_set_visible(item: *mut c_void, visible: bool);
    fn obs_sceneitem_visibility_set_user_visible(item: *mut c_void, visible: bool);
}

/// The real active-child insertion operation (its return is intentionally ignored).
pub type AddActiveChild = unsafe extern "C" fn(*mut c_void, *mut c_void) -> bool;
/// The real active-child removal operation.
pub type RemoveActiveChild = unsafe extern "C" fn(*mut c_void, *mut c_void);

struct Item {
    item: NonNull<c_void>,
    add: AddActiveChild,
    remove: RemoveActiveChild,
}

impl VisibilityItem for Item {
    fn lock(&mut self) {
        // SAFETY: the caller supplies a live item and its initialized actions mutex.
        unsafe { obs_sceneitem_visibility_lock(self.item.as_ptr()) }
    }

    fn unlock(&mut self) {
        // SAFETY: the safe core acquired this item's mutex on this thread.
        unsafe { obs_sceneitem_visibility_unlock(self.item.as_ptr()) }
    }

    fn clear_actions(&mut self) {
        // SAFETY: the live item's queue is protected by the held actions mutex.
        unsafe { obs_sceneitem_visibility_clear(self.item.as_ptr()) }
    }

    fn active_refs(&mut self) -> i128 {
        // SAFETY: a live item; the adapter performs the original atomic load.
        i128::from(unsafe { obs_sceneitem_visibility_refs(self.item.as_ptr()) })
    }

    fn add_active_child(&mut self) {
        // SAFETY: the caller retains the scene and both sources; the adapter uses
        // their original addresses. The original helper ignores insertion failure.
        unsafe {
            (self.add)(
                obs_sceneitem_visibility_parent_source(self.item.as_ptr()),
                obs_sceneitem_visibility_source(self.item.as_ptr()),
            );
        }
    }

    fn remove_active_child(&mut self) {
        // SAFETY: the caller retains the scene and both sources, as in C.
        unsafe {
            (self.remove)(
                obs_sceneitem_visibility_parent_source(self.item.as_ptr()),
                obs_sceneitem_visibility_source(self.item.as_ptr()),
            );
        }
    }

    fn set_active_refs(&mut self, visible: bool) {
        // SAFETY: the live item's refs are normalized with the original atomic store.
        unsafe { obs_sceneitem_visibility_set_refs(self.item.as_ptr(), visible) }
    }

    fn set_visible(&mut self, visible: bool) {
        // SAFETY: a live item, one original atomic store under the actions mutex.
        unsafe { obs_sceneitem_visibility_set_visible(self.item.as_ptr(), visible) }
    }

    fn set_user_visible(&mut self, visible: bool) {
        // SAFETY: a live item, one original atomic store under the actions mutex.
        unsafe { obs_sceneitem_visibility_set_user_visible(self.item.as_ptr(), visible) }
    }
}

/// Reconcile through supplied active-child operations so parity can record them.
///
/// # Safety
/// A non-NULL item, its parent scene and both sources must remain live throughout
/// the call. The caller must obey the same scene-lock contract as the C helper.
/// The callbacks must accept those source pointers as the libobs functions do.
pub unsafe fn set_visibility_with(
    item: *mut c_void,
    visible: bool,
    add: AddActiveChild,
    remove: RemoveActiveChild,
) {
    if let Some(item) = NonNull::new(item) {
        item_set_visibility::set_visibility(&mut Item { item, add, remove }, visible);
    }
}

/// Replace the private `set_visibility` helper; NULL is a defensive no-op.
///
/// # Safety
/// As [`set_visibility_with`], using the production libobs active-child operations.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_set_visibility(item: *mut c_void, visible: bool) {
    // SAFETY: forward the caller's live-item contract and real libobs operations.
    unsafe {
        set_visibility_with(
            item,
            visible,
            obs_source_add_active_child,
            obs_source_remove_active_child,
        )
    }
}
