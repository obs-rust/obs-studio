//! Scalar bounds fields, read under the captured parent's existing C mutex.
use super::{
    item_state_getters::SceneItemPrefix as StatePrefix, scene_bounds_place::SceneItemCrop,
};
use crate::item_bounds_getters;
use core::ffi::c_void;
use core::ptr;
use obs_graphics::ffi::{matrix4::matrix4, vec2::vec2};

/// Private C item prefix. Only the requested field is read, never this whole
/// prefix: its final alignment padding may overlap the opaque C suffix.
#[repr(C)]
pub struct SceneItemPrefix {
    pub state: StatePrefix,
    pub item_render: *mut c_void,
    pub crop: SceneItemCrop,
    pub absolute_coordinates: bool,
    pub pos: vec2,
    pub scale: vec2,
    pub scale_ref: vec2,
    pub rot: f32,
    pub align: u32,
    pub last_width: u32,
    pub last_height: u32,
    pub output_scale: vec2,
    pub scale_filter: u32,
    pub blend_method: u32,
    pub blend_type: u32,
    pub box_transform: matrix4,
    pub box_scale: vec2,
    pub draw_transform: matrix4,
    pub bounds_type: u32,
    pub bounds_align: u32,
    pub bounds: vec2,
    pub crop_to_bounds: bool,
}

unsafe extern "C" {
    fn obs_sceneitem_fields_lock(item: *const SceneItemPrefix) -> *mut c_void;
    fn obs_sceneitem_fields_unlock(scene: *mut c_void);
}

/// Return the stored bounds type, or OBS_BOUNDS_NONE for NULL.
///
/// # Safety
/// A non-null item and its captured parent must remain live during the call,
/// under the same lifecycle contract as the original C getter.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_get_bounds_type(item: *const SceneItemPrefix) -> u32 {
    let value = if item.is_null() {
        None
    } else {
        // SAFETY: C captures the live parent and takes precisely its video mutex.
        let scene = unsafe { obs_sceneitem_fields_lock(item) };
        // SAFETY: read only the requested field while the C mutex is held.
        let value = unsafe { ptr::addr_of!((*item).bounds_type).read() };
        // SAFETY: unlock the captured parent, including the detached NULL case.
        unsafe { obs_sceneitem_fields_unlock(scene) };
        Some(value)
    };
    item_bounds_getters::bounds_type(value)
}

/// Return the stored bounds alignment, or zero for NULL.
///
/// # Safety
/// The item and parent must satisfy the original C getter's lifecycle contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_get_bounds_alignment(item: *const SceneItemPrefix) -> u32 {
    let value = if item.is_null() {
        None
    } else {
        // SAFETY: the caller provides a live item and parent; C owns the mutex.
        let scene = unsafe { obs_sceneitem_fields_lock(item) };
        // SAFETY: read only this field under the captured parent's video mutex.
        let value = unsafe { ptr::addr_of!((*item).bounds_align).read() };
        // SAFETY: every locking path unlocks the same captured scene.
        unsafe { obs_sceneitem_fields_unlock(scene) };
        Some(value)
    };
    item_bounds_getters::bounds_alignment(value)
}

/// Return the stored crop flag, or false for NULL.
///
/// # Safety
/// The item and parent must satisfy the original C getter's lifecycle contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_get_bounds_crop(item: *const SceneItemPrefix) -> bool {
    let value = if item.is_null() {
        None
    } else {
        // SAFETY: the caller provides a live item and parent; C owns the mutex.
        let scene = unsafe { obs_sceneitem_fields_lock(item) };
        // SAFETY: read only this flag while the existing C video mutex is held.
        let value = unsafe { ptr::addr_of!((*item).crop_to_bounds).read() };
        // SAFETY: every locking path unlocks the same captured scene.
        unsafe { obs_sceneitem_fields_unlock(scene) };
        Some(value)
    };
    item_bounds_getters::bounds_crop(value)
}
