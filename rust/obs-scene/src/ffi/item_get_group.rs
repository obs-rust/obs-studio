//! Parent-group query over the real C full lock, walk and group-flag seams.
use crate::item_get_group::{self, Query};
use core::ffi::c_void;
use core::ptr::{self, NonNull};

unsafe extern "C" {
    fn obs_scene_atomic_full_lock(scene: *mut c_void);
    fn obs_scene_atomic_full_unlock(scene: *mut c_void);
    fn obs_sceneitem_parent_group_lookup(scene: *mut c_void, item: *mut c_void) -> *mut c_void;
    fn obs_sceneitem_group_scene_flag(item: *const c_void) -> *const bool;
}

struct Live {
    scene: NonNull<c_void>,
    item: NonNull<c_void>,
}

impl Query for Live {
    type Group = NonNull<c_void>;
    fn is_group(&mut self) -> bool {
        // SAFETY: the item is non-null and the caller keeps it readable.
        unsafe { *obs_sceneitem_group_scene_flag(self.item.as_ptr()) }
    }
    fn lock(&mut self) {
        // SAFETY: the scene is non-null and live under the caller's contract.
        unsafe { obs_scene_atomic_full_lock(self.scene.as_ptr()) };
    }
    fn lookup(&mut self) -> Option<Self::Group> {
        // SAFETY: the full lock is held; C walks the scene's item list.
        NonNull::new(unsafe {
            obs_sceneitem_parent_group_lookup(self.scene.as_ptr(), self.item.as_ptr())
        })
    }
    fn unlock(&mut self) {
        // SAFETY: paired with lock on the same scene.
        unsafe { obs_scene_atomic_full_unlock(self.scene.as_ptr()) };
    }
}

/// The group item holding `item` in `scene`, borrowed (no reference taken).
///
/// # Safety
/// Non-null arguments must be a live scene and a readable scene item, as in C.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_get_group(
    scene: *mut c_void,
    item: *mut c_void,
) -> *mut c_void {
    let (Some(scene), Some(item)) = (NonNull::new(scene), NonNull::new(item)) else {
        return ptr::null_mut();
    };
    item_get_group::get_group(Some(&mut Live { scene, item }))
        .map_or(ptr::null_mut(), NonNull::as_ptr)
}
