//! Read-only C ABI for the selected and locked scene item getters.
//! Only the prefix through `locked` is described; the remaining item is opaque.
//! Each shim reads only its requested field, without borrowing the whole item.
use crate::item_state_getters;
use core::ffi::{c_long, c_void};

/// Prefix of the private C `obs_scene_item`, ending before `item_render`.
/// Real-header tests pin its span, alignment and every field offset.
#[repr(C)]
pub struct SceneItemPrefix {
    pub refs: c_long,
    pub removed: bool,
    pub is_group: bool,
    pub is_scene: bool,
    pub update_transform: bool,
    pub update_group_resize: bool,
    pub id: i64,
    pub parent: *mut c_void,
    pub source: *mut c_void,
    pub active_refs: c_long,
    pub defer_update: c_long,
    pub defer_group_resize: c_long,
    pub user_visible: bool,
    pub visible: bool,
    pub selected: bool,
    pub locked: bool,
}

/// Read the stored selected flag; null means false.
///
/// # Safety
/// A non-null item must be a live C scene item whose selected field is readable
/// under the same synchronization contract as the original C getter.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_selected(item: *const SceneItemPrefix) -> bool {
    let value = if item.is_null() {
        None
    } else {
        // SAFETY: the caller supplies a live item; read only the requested field.
        Some(unsafe { core::ptr::addr_of!((*item).selected).read() })
    };
    item_state_getters::selected(value)
}

/// Read the stored locked flag; null means false.
///
/// # Safety
/// A non-null item must be a live C scene item whose locked field is readable
/// under the same synchronization contract as the original C getter.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_locked(item: *const SceneItemPrefix) -> bool {
    let value = if item.is_null() {
        None
    } else {
        // SAFETY: the caller supplies a live item; read only the requested field.
        Some(unsafe { core::ptr::addr_of!((*item).locked).read() })
    };
    item_state_getters::locked(value)
}
