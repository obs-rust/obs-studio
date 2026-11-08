//! Synchronized scale reads through the existing captured-parent C boundary.
use super::item_bounds_getters::SceneItemPrefix;
use crate::item_scale_getter::read_scale;
use core::{ffi::c_void, ptr};
use obs_graphics::ffi::vec2::vec2;

unsafe extern "C" {
    fn obs_sceneitem_fields_lock(item: *const SceneItemPrefix) -> *mut c_void;
    fn obs_sceneitem_fields_unlock(scene: *mut c_void);
    fn obs_sceneitem_scale_canvas_read(output: *mut vec2, item: *const SceneItemPrefix);
}

/// Read scale, retaining the C canvas conversion for relative coordinates.
/// NULL items leave output untouched, including a NULL output.
///
/// # Safety
/// A non-null item and its captured parent must remain live as in the original
/// C getter. Its output must point to writable vec2 storage and may alias scale.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_get_scale(item: *const SceneItemPrefix, output: *mut vec2) {
    if item.is_null() {
        return;
    }
    // SAFETY: C captures the live parent and takes exactly its video mutex.
    let scene = unsafe { obs_sceneitem_fields_lock(item) };
    // SAFETY: read only the two required fields under the captured parent's lock.
    let (stored, absolute) = unsafe {
        (
            ptr::addr_of!((*item).scale).read(),
            ptr::addr_of!((*item).absolute_coordinates).read(),
        )
    };
    let mut bits = [0; 2];
    read_scale(
        Some(([stored.x.to_bits(), stored.y.to_bits()], absolute)),
        || {
            let mut converted = vec2::default();
            // SAFETY: live C item and writable local output, with its mutex held.
            unsafe { obs_sceneitem_scale_canvas_read(&raw mut converted, item) };
            [converted.x.to_bits(), converted.y.to_bits()]
        },
        &mut bits,
    );
    // SAFETY: the caller supplies output storage; all inputs were read before
    // writing, so aliasing the item's stored scale has the original behavior.
    unsafe {
        output.write(vec2 {
            x: f32::from_bits(bits[0]),
            y: f32::from_bits(bits[1]),
        });
        obs_sceneitem_fields_unlock(scene);
    }
}
