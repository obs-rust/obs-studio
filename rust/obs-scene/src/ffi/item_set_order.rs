//! `obs_sceneitem_set_order()` over the real C seams: the parent field
//! (`obs_sceneitem_parent_fields`), the scene reference
//! (`obs_scene_get_ref`, `obs_scene_release`), the full lock
//! (`obs_scene_atomic_full_lock`), the links (`obs_scene_lookup_next` and
//! the private adapters of `obs-scene-item-set-order-fields.c.h`, which run
//! `detach_sceneitem()`, `attach_sceneitem()` and `signal_reorder()` of
//! `obs-scene.c.h`). `struct obs_scene_item` stays opaque.
use crate::item_set_order::{self, SetOrderScene};
use core::ffi::{c_int, c_void};
use core::ptr::{self, NonNull};

unsafe extern "C" {
    fn obs_sceneitem_parent_fields(item: *const c_void) -> *const *mut c_void;
    fn obs_scene_get_ref(scene: *mut c_void) -> *mut c_void;
    fn obs_scene_release(scene: *mut c_void);
    fn obs_scene_atomic_full_lock(scene: *mut c_void);
    fn obs_scene_atomic_full_unlock(scene: *mut c_void);
    fn obs_scene_lookup_next(item: *mut c_void) -> *mut c_void;
    fn obs_sceneitem_set_order_prev(item: *const c_void) -> *mut c_void;
    fn obs_sceneitem_set_order_detach(item: *mut c_void);
    fn obs_sceneitem_set_order_attach(scene: *mut c_void, item: *mut c_void, prev: *mut c_void);
    fn obs_sceneitem_set_order_signal(item: *mut c_void);
}

type Handle = NonNull<c_void>;

struct Live;

impl SetOrderScene for Live {
    type Item = Handle;
    type Scene = Handle;
    fn parent_ref(&mut self, item: Handle) -> Option<Handle> {
        // SAFETY: the caller keeps the non-null item readable; a NULL parent
        // gives NULL from obs_scene_get_ref, as in C.
        NonNull::new(unsafe { obs_scene_get_ref(*obs_sceneitem_parent_fields(item.as_ptr())) })
    }
    fn lock(&mut self, scene: Handle) {
        // SAFETY: the core holds a reference to the scene.
        unsafe { obs_scene_atomic_full_lock(scene.as_ptr()) };
    }
    fn next(&mut self, item: Handle) -> Option<Handle> {
        // SAFETY: the full lock is held and `item` is in the scene's list.
        NonNull::new(unsafe { obs_scene_lookup_next(item.as_ptr()) })
    }
    fn prev(&mut self, item: Handle) -> Option<Handle> {
        // SAFETY: the full lock is held and `item` is in the scene's list.
        NonNull::new(unsafe { obs_sceneitem_set_order_prev(item.as_ptr()) })
    }
    fn detach(&mut self, item: Handle) {
        // SAFETY: the full lock is held and the item is linked in its parent.
        unsafe { obs_sceneitem_set_order_detach(item.as_ptr()) };
    }
    fn attach(&mut self, scene: Handle, item: Handle, after: Option<Handle>) {
        let after = after.map_or(ptr::null_mut(), NonNull::as_ptr);
        // SAFETY: the full lock is held; the item was just detached and
        // `after` is NULL or an item of the scene.
        unsafe { obs_sceneitem_set_order_attach(scene.as_ptr(), item.as_ptr(), after) };
    }
    fn unlock(&mut self, scene: Handle) {
        // SAFETY: paired with lock on the same scene.
        unsafe { obs_scene_atomic_full_unlock(scene.as_ptr()) };
    }
    fn signal_reorder(&mut self, item: Handle) {
        // SAFETY: the item is attached again and its parent referenced.
        unsafe { obs_sceneitem_set_order_signal(item.as_ptr()) };
    }
    fn release(&mut self, scene: Handle) {
        // SAFETY: releases the reference parent_ref took.
        unsafe { obs_scene_release(scene.as_ptr()) };
    }
}

/// Move `item` up, down, to the top or to the bottom of its parent scene.
///
/// # Safety
/// A non-null item must be readable and, when it has a parent, linked in
/// that scene's list, as in C.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_set_order(item: *mut c_void, movement: c_int) {
    item_set_order::set_order(&mut Live, NonNull::new(item), movement);
}
