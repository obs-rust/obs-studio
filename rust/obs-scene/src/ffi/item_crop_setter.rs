//! Crop store under the existing captured-parent C video mutex, then the
//! transform flag after the unlock, as the original setter does.
use super::{item_crop_getter::SceneItemPrefix, scene_bounds_place::SceneItemCrop};
use crate::item_crop_setter::{self, Missing};
use core::ffi::{c_char, c_int, c_void};
use core::ptr;
use core::sync::atomic::{AtomicBool, Ordering};

unsafe extern "C" {
    fn obs_sceneitem_fields_lock(
        item: *const super::item_bounds_getters::SceneItemPrefix,
    ) -> *mut c_void;
    fn obs_sceneitem_fields_unlock(scene: *mut c_void);
    fn blog(level: c_int, format: *const c_char, ...);
}

fn sides(crop: SceneItemCrop) -> [i32; 4] {
    [crop.left, crop.top, crop.right, crop.bottom]
}

/// Store the clamped crop unless the raw input equals the stored crop, then
/// mark the transform, preserving the original null diagnostics and order.
///
/// # Safety
/// Non-null pointers must reference live C objects of their declared types;
/// the captured parent must remain live throughout the original C lock
/// contract. Concurrent accesses to update_transform must be atomic.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_set_crop(
    item: *mut SceneItemPrefix,
    crop: *const SceneItemCrop,
) {
    if let Err(missing) = item_crop_setter::validate(!item.is_null(), !crop.is_null()) {
        let name = match missing {
            Missing::Item => c"item",
            Missing::Crop => c"crop",
        };
        // SAFETY: the format and both variadic arguments are static C strings.
        unsafe {
            blog(
                400,
                c"%s: Null '%s' parameter".as_ptr(),
                c"obs_sceneitem_set_crop".as_ptr(),
                name.as_ptr(),
            )
        };
        return;
    }
    // SAFETY: validated live input; read before the lock, as C clamps first.
    let clamped = item_crop_setter::clamp(sides(unsafe { crop.read() }));
    // SAFETY: validated live item; C snapshots and locks its captured parent.
    let scene = unsafe { obs_sceneitem_fields_lock(item.cast()) };
    // SAFETY: the input is read again under the lock, where C compares it.
    let input = sides(unsafe { crop.read() });
    // SAFETY: read and write only crop while holding the parent's video mutex.
    let mut stored = sides(unsafe { ptr::addr_of!((*item).crop).read() });
    let changed = item_crop_setter::store(&mut stored, input, clamped);
    if changed {
        // SAFETY: as above; the item's crop is exclusively ours under the lock.
        unsafe {
            ptr::addr_of_mut!((*item).crop).write(SceneItemCrop {
                left: stored[0],
                top: stored[1],
                right: stored[2],
                bottom: stored[3],
            })
        };
    }
    // SAFETY: unlock the same captured parent, including the detached null path.
    unsafe { obs_sceneitem_fields_unlock(scene) };
    if changed {
        // SAFETY: AtomicBool has bool's size and alignment; from_ptr borrows
        // only this field, which every other access touches atomically.
        let dirty =
            unsafe { AtomicBool::from_ptr(ptr::addr_of_mut!((*item).state.update_transform)) };
        dirty.swap(true, Ordering::SeqCst);
    }
}
