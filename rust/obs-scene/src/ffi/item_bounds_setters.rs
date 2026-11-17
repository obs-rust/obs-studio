//! The bounds setters of `obs-scene-item-bounds-setters.c.h` over the bounds
//! getters' layout-checked item prefix. Each one takes the existing item
//! fields lock, stores its field, unlocks, then asks C to update the
//! transform (`obs-scene-item-bounds-setters-fields.c.h` wraps the scene's
//! own dispatch and its relative size conversion), as the C does.
use super::item_bounds_getters::SceneItemPrefix;
use crate::item_bounds_setters::{self, Fields, Skip, Store};
use core::ffi::{c_char, c_int, c_void};
use core::ptr;
use obs_graphics::ffi::vec2::vec2;

unsafe extern "C" {
    fn obs_sceneitem_fields_lock(item: *const SceneItemPrefix) -> *mut c_void;
    fn obs_sceneitem_fields_unlock(scene: *mut c_void);
    fn obs_sceneitem_bounds_size_from_absolute(
        dst: *mut vec2,
        v: *const vec2,
        item: *const SceneItemPrefix,
    );
    fn obs_sceneitem_bounds_update_transform(item: *mut SceneItemPrefix);
    fn blog(level: c_int, format: *const c_char, ...);
}

/// A live, non-null item and the parent its lock captured.
struct Item {
    item: *mut SceneItemPrefix,
    scene: *mut c_void,
}

impl Fields for Item {
    fn lock(&mut self) {
        // SAFETY: `Item` is only built over a live item (see `set`); C
        // captures its parent and takes that scene's video mutex.
        self.scene = unsafe { obs_sceneitem_fields_lock(self.item) };
    }
    fn unlock(&mut self) {
        // SAFETY: unlock exactly the captured parent, NULL for a detached item.
        unsafe { obs_sceneitem_fields_unlock(self.scene) };
    }
    fn update(&mut self) {
        // SAFETY: the live item, after the unlock, as `do_update_transform`.
        unsafe { obs_sceneitem_bounds_update_transform(self.item) };
    }
}

/// Lock, `write` the item, unlock, update.
///
/// # Safety
/// `item` must be a live, non-null scene item whose parent stays live for
/// the call, under the original setters' lifecycle contract.
unsafe fn set(item: *mut SceneItemPrefix, write: impl FnOnce(*mut SceneItemPrefix)) {
    let mut fields = Item {
        item,
        scene: ptr::null_mut(),
    };
    item_bounds_setters::set(&mut fields, |fields| write(fields.item));
}

/// `valid_bounds_type`, shared with `obs-scene.c.h`'s scene and transform
/// state loaders: a value outside the enum is `OBS_BOUNDS_NONE`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_scene_bounds_valid_type(value: i64) -> u32 {
    item_bounds_setters::valid_bounds_type(value)
}

/// Set the bounds type; a value outside the enum stores `OBS_BOUNDS_NONE`.
///
/// The enum arrives as 32 bits. C widens it with or without sign by
/// compiler; either way only 0 to 6 are inside the enum, so zero extension
/// decides the same.
///
/// # Safety
/// A non-null item and its parent must stay live for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_set_bounds_type(item: *mut SceneItemPrefix, value: u32) {
    if item.is_null() {
        return;
    }
    // SAFETY: a non-null, live item (caller contract).
    unsafe {
        set(item, |item| {
            // Checked under the lock, where the C calls valid_bounds_type.
            let kind = item_bounds_setters::valid_bounds_type(i64::from(value));
            // SAFETY: only this field is written, under the fields lock.
            ptr::addr_of_mut!((*item).bounds_type).write(kind);
        });
    }
}

/// Set the bounds alignment, every bit as given.
///
/// # Safety
/// A non-null item and its parent must stay live for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_set_bounds_alignment(
    item: *mut SceneItemPrefix,
    alignment: u32,
) {
    if item.is_null() {
        return;
    }
    // SAFETY: a non-null, live item (caller contract).
    unsafe {
        set(item, |item| {
            // SAFETY: only this field is written, under the fields lock.
            ptr::addr_of_mut!((*item).bounds_align).write(alignment);
        });
    }
}

/// Set whether the item is cropped to its bounds.
///
/// # Safety
/// A non-null item and its parent must stay live for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_set_bounds_crop(item: *mut SceneItemPrefix, crop: bool) {
    if item.is_null() {
        return;
    }
    // SAFETY: a non-null, live item (caller contract).
    unsafe {
        set(item, |item| {
            // SAFETY: only this field is written, under the fields lock.
            ptr::addr_of_mut!((*item).crop_to_bounds).write(crop);
        });
    }
}

/// Set the bounds size, converted from absolute coordinates unless the item
/// uses them. A NULL `bounds` is logged and stores nothing, before the lock
/// (the C read through it: an intentional difference). A NULL item does
/// nothing, silently, as before.
///
/// # Safety
/// A non-null item and its parent must stay live for the call, and a
/// non-null `bounds` must point to a readable `struct vec2`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_set_bounds(item: *mut SceneItemPrefix, bounds: *const vec2) {
    match item_bounds_setters::check(!item.is_null(), !bounds.is_null()) {
        Ok(()) => {}
        Err(Skip::Item) => return,
        Err(Skip::Bounds) => {
            // SAFETY: the format and both arguments are static C strings;
            // LOG_DEBUG, as obs_ptr_valid logs it.
            unsafe {
                blog(
                    400,
                    c"%s: Null '%s' parameter".as_ptr(),
                    c"obs_sceneitem_set_bounds".as_ptr(),
                    c"bounds".as_ptr(),
                );
            }
            return;
        }
    }
    // SAFETY: both pointers checked non-null; live by the caller contract.
    unsafe {
        set(item, |item| {
            // SAFETY: under the fields lock, as the C reads it.
            let absolute = ptr::addr_of!((*item).absolute_coordinates).read();
            let dst = ptr::addr_of_mut!((*item).bounds);
            match item_bounds_setters::store(absolute) {
                // SAFETY: C converts through the item's scene size.
                Store::Convert => obs_sceneitem_bounds_size_from_absolute(dst, bounds, item),
                Store::Copy => {
                    // SAFETY: vec2_copy's order: x, then y, each read then
                    // written, so an aliased `bounds` behaves the same.
                    ptr::addr_of_mut!((*dst).x).write(ptr::addr_of!((*bounds).x).read());
                    ptr::addr_of_mut!((*dst).y).write(ptr::addr_of!((*bounds).y).read());
                }
            }
        });
    }
}
