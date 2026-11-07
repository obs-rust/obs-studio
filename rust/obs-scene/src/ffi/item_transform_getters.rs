//! Transform scalars read under the captured parent's existing C mutex.
use super::item_bounds_getters::SceneItemPrefix;
use crate::item_transform_getters;
use core::{ffi::c_void, ptr};

unsafe extern "C" {
    fn obs_sceneitem_fields_lock(item: *const SceneItemPrefix) -> *mut c_void;
    fn obs_sceneitem_fields_unlock(scene: *mut c_void);
}

/// Read stored rotation, or positive zero for NULL.
///
/// # Safety
/// A non-null item and its captured parent must remain live during the call,
/// under the same lifecycle contract as the original C getter.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_get_rot(item: *const SceneItemPrefix) -> f32 {
    let value = if item.is_null() {
        None
    } else {
        // SAFETY: C captures the live parent and takes precisely its video mutex.
        let scene = unsafe { obs_sceneitem_fields_lock(item) };
        // SAFETY: read only this field under the captured parent's video mutex.
        let value = unsafe { ptr::addr_of!((*item).rot).read() };
        // SAFETY: every locking path unlocks the same captured scene.
        unsafe { obs_sceneitem_fields_unlock(scene) };
        Some(value)
    };
    item_transform_getters::rotation(value)
}

/// Read stored alignment, or zero for NULL.
///
/// # Safety
/// The item and parent must satisfy the original C getter's lifecycle contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_get_alignment(item: *const SceneItemPrefix) -> u32 {
    let value = if item.is_null() {
        None
    } else {
        // SAFETY: the caller provides a live item and parent; C owns the mutex.
        let scene = unsafe { obs_sceneitem_fields_lock(item) };
        // SAFETY: read only this field under the captured parent's video mutex.
        let value = unsafe { ptr::addr_of!((*item).align).read() };
        // SAFETY: every locking path unlocks the same captured scene.
        unsafe { obs_sceneitem_fields_unlock(scene) };
        Some(value)
    };
    item_transform_getters::alignment(value)
}
