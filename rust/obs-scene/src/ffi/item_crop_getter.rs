//! Crop read under the existing captured-parent C video mutex.
use super::{
    item_state_getters::SceneItemPrefix as StatePrefix, scene_bounds_place::SceneItemCrop,
};
use core::ffi::{c_char, c_int, c_void};
use core::ptr;

/// Private item prefix through crop; never read the prefix as a whole.
#[repr(C)]
pub struct SceneItemPrefix {
    pub state: StatePrefix,
    pub item_render: *mut c_void,
    pub crop: SceneItemCrop,
}

unsafe extern "C" {
    fn obs_sceneitem_fields_lock(
        item: *const super::item_bounds_getters::SceneItemPrefix,
    ) -> *mut c_void;
    fn obs_sceneitem_fields_unlock(scene: *mut c_void);
    fn blog(level: c_int, format: *const c_char, ...);
}

/// Copy the stored crop, preserving the original null diagnostics and order.
///
/// # Safety
/// Non-null pointers must reference live C objects of their declared types;
/// the captured parent must remain live throughout the original C lock contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_get_crop(
    item: *const SceneItemPrefix,
    crop: *mut SceneItemCrop,
) {
    let mut output = [0; 4];
    let validation = crate::item_crop_getter::get(
        (!item.is_null()).then_some([0; 4]),
        (!crop.is_null()).then_some(&mut output),
    );
    if let Err(missing) = validation {
        let name = match missing {
            crate::item_crop_getter::Missing::Item => c"item",
            crate::item_crop_getter::Missing::Crop => c"crop",
        };
        // SAFETY: the format and both variadic arguments are static C strings.
        unsafe {
            blog(
                400,
                c"%s: Null '%s' parameter".as_ptr(),
                c"obs_sceneitem_get_crop".as_ptr(),
                name.as_ptr(),
            )
        };
        return;
    }
    // SAFETY: validated live item; C snapshots and locks its captured parent.
    let scene = unsafe { obs_sceneitem_fields_lock(item.cast()) };
    // SAFETY: read only crop while holding precisely the parent's video mutex.
    let value = unsafe { ptr::addr_of!((*item).crop).read() };
    // The validated destination exists; the safe core copies all four integers.
    let _ = crate::item_crop_getter::get(
        Some([value.left, value.top, value.right, value.bottom]),
        Some(&mut output),
    );
    // SAFETY: output is valid and writable; write under the same lock as C.
    unsafe {
        crop.write(SceneItemCrop {
            left: output[0],
            top: output[1],
            right: output[2],
            bottom: output[3],
        })
    };
    // SAFETY: unlock the same captured parent, including the detached null path.
    unsafe { obs_sceneitem_fields_unlock(scene) };
}
