//! Scene-item order position over the real C parent, head, link and full-lock
//! seams.
use crate::item_order::item_attached;
use crate::item_order_position::{self, ParentList};
use core::ffi::{c_int, c_void};
use core::ptr::NonNull;

unsafe extern "C" {
    fn obs_scene_atomic_full_lock(scene: *mut c_void);
    fn obs_scene_atomic_full_unlock(scene: *mut c_void);
    fn obs_sceneitem_parent_fields(item: *const c_void) -> *const *mut c_void;
    fn obs_scene_item_order_position_first(scene: *const c_void) -> *mut c_void;
    fn obs_scene_lookup_next(item: *mut c_void) -> *mut c_void;
}

type Handle = NonNull<c_void>;

struct Live {
    scene: Handle,
    item: Handle,
}

impl ParentList for Live {
    type Item = Handle;
    fn target(&self) -> Handle {
        self.item
    }
    fn head(&mut self) -> Option<Handle> {
        // SAFETY: the parent scene is non-null and live under the caller's
        // contract; C reads its head before the lock, as here.
        NonNull::new(unsafe { obs_scene_item_order_position_first(self.scene.as_ptr()) })
    }
    fn lock(&mut self) {
        // SAFETY: the scene is non-null and live under the caller's contract.
        unsafe { obs_scene_atomic_full_lock(self.scene.as_ptr()) };
    }
    fn next(&mut self, item: Handle) -> Option<Handle> {
        // SAFETY: the full lock is held and `item` is linked in the scene.
        NonNull::new(unsafe { obs_scene_lookup_next(item.as_ptr()) })
    }
    fn unlock(&mut self) {
        // SAFETY: paired with lock on the same scene.
        unsafe { obs_scene_atomic_full_unlock(self.scene.as_ptr()) };
    }
}

/// Zero-based position of `item` in its parent's list (0 for NULL or
/// detached).
///
/// # Safety
/// A non-null item must be readable and its parent scene live, as in C.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_get_order_position(item: *mut c_void) -> c_int {
    let item = NonNull::new(item);
    let parent = item.and_then(|item| {
        // SAFETY: the caller keeps the non-null item and its parent field readable.
        NonNull::new(unsafe { *obs_sceneitem_parent_fields(item.as_ptr()) })
    });
    // The attached-item predicate of #2899: a NULL or detached item is 0
    // before the head read and the lock.
    let attached = item_attached(item.is_some(), parent.is_some());
    let mut list = match (item, parent) {
        (Some(item), Some(scene)) if attached => Some(Live { scene, item }),
        _ => None,
    };
    item_order_position::order_position(list.as_mut())
}
