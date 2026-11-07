//! Read-only item ID C ABI. Only the prefix through `id` is described.
//! Read only `id`, without borrowing the item or its opaque suffix.
use core::ffi::{c_char, c_int, c_long};

/// Private scene-item prefix ending immediately before `parent`.
/// Size, alignment and every offset are checked against the real C header.
#[repr(C)]
pub struct SceneItemIdPrefix {
    pub refs: c_long,
    pub removed: bool,
    pub is_group: bool,
    pub is_scene: bool,
    pub update_transform: bool,
    pub update_group_resize: bool,
    pub id: i64,
}

unsafe extern "C" {
    fn blog(level: c_int, format: *const c_char, ...);
}

/// Read a stored signed ID; null logs the original diagnostic and returns zero.
///
/// # Safety
/// A non-null item must be live with its ID readable under the original C
/// getter's synchronization contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_get_id(item: *const SceneItemIdPrefix) -> i64 {
    let value = if item.is_null() {
        None
    } else {
        // SAFETY: caller supplies a live item; read only its initialized ID.
        Some(unsafe { core::ptr::addr_of!((*item).id).read() })
    };
    crate::item_id::get_id(value, || {
        // SAFETY: the format and both string arguments are static C strings.
        unsafe {
            blog(
                400,
                c"%s: Null '%s' parameter".as_ptr(),
                c"obs_sceneitem_get_id".as_ptr(),
                c"item".as_ptr(),
            )
        };
    })
}
