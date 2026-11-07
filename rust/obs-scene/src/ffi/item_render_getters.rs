//! Read-only scene item render getters. The suffix after blend_type is opaque.
use core::ffi::{c_char, c_int, c_long, c_void};

/// Private C prefix. Every offset and its span/alignment are measured from C.
#[repr(C)]
pub struct SceneItemRenderPrefix {
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
    pub item_render: *mut c_void,
    pub crop: [i32; 4],
    pub absolute_coordinates: bool,
    pub pos: [f32; 2],
    pub scale: [f32; 2],
    pub scale_ref: [f32; 2],
    pub rot: f32,
    pub align: u32,
    pub last_width: u32,
    pub last_height: u32,
    pub output_scale: [f32; 2],
    pub scale_filter: u32,
    pub blend_method: u32,
    pub blend_type: u32,
}

unsafe extern "C" {
    fn blog(level: c_int, format: *const c_char, ...);
}

/// Return the stored value, or the original null default and diagnostic.
///
/// # Safety
/// A non-null item must be live and its requested field readable under the
/// original C getter's synchronization contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_get_scale_filter(item: *const SceneItemRenderPrefix) -> u32 {
    let value = if item.is_null() {
        None
    } else {
        // SAFETY: caller supplies a live item; read only the requested field.
        Some(unsafe { core::ptr::addr_of!((*item).scale_filter).read() })
    };
    crate::item_render_getters::get(value, || {
        // SAFETY: format and both arguments are static C strings.
        unsafe {
            blog(
                400,
                c"%s: Null '%s' parameter".as_ptr(),
                c"obs_sceneitem_get_scale_filter".as_ptr(),
                c"item".as_ptr(),
            )
        };
    })
}

/// Return the stored value, or the original null default and diagnostic.
///
/// # Safety
/// A non-null item must be live and its requested field readable under the
/// original C getter's synchronization contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_get_blending_method(
    item: *const SceneItemRenderPrefix,
) -> u32 {
    let value = if item.is_null() {
        None
    } else {
        // SAFETY: caller supplies a live item; read only the requested field.
        Some(unsafe { core::ptr::addr_of!((*item).blend_method).read() })
    };
    crate::item_render_getters::get(value, || {
        // SAFETY: format and both arguments are static C strings.
        unsafe {
            blog(
                400,
                c"%s: Null '%s' parameter".as_ptr(),
                c"obs_sceneitem_get_blending_method".as_ptr(),
                c"item".as_ptr(),
            )
        };
    })
}

/// Return the stored value, or the original null default and diagnostic.
///
/// # Safety
/// A non-null item must be live and its requested field readable under the
/// original C getter's synchronization contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_get_blending_mode(
    item: *const SceneItemRenderPrefix,
) -> u32 {
    let value = if item.is_null() {
        None
    } else {
        // SAFETY: caller supplies a live item; read only the requested field.
        Some(unsafe { core::ptr::addr_of!((*item).blend_type).read() })
    };
    crate::item_render_getters::get(value, || {
        // SAFETY: format and both arguments are static C strings.
        unsafe {
            blog(
                400,
                c"%s: Null '%s' parameter".as_ptr(),
                c"obs_sceneitem_get_blending_mode".as_ptr(),
                c"item".as_ptr(),
            )
        };
    })
}
