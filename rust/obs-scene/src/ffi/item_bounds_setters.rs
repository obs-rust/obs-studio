//! C bounds setters over the already layout-checked private item prefix.
use super::item_bounds_getters::SceneItemPrefix;
use crate::item_bounds_setters::{self, Target, Write};
use core::{ffi::c_void, ptr};
use obs_graphics::{ffi::vec2::vec2, vec2::Vec2};

unsafe extern "C" {
    fn obs_sceneitem_fields_lock(item: *const SceneItemPrefix) -> *mut c_void;
    fn obs_sceneitem_fields_unlock(scene: *mut c_void);
    fn obs_sceneitem_bounds_canvas_height(item: *const SceneItemPrefix) -> f32;
    fn obs_sceneitem_bounds_update(item: *mut SceneItemPrefix);
}
struct Item {
    item: *mut SceneItemPrefix,
    scene: *mut c_void,
    bounds: Option<*const vec2>,
}
impl Target for Item {
    fn lock(&mut self) {
        // SAFETY: construction requires a live non-null item and parent.
        self.scene = unsafe { obs_sceneitem_fields_lock(self.item) };
    }
    fn write(&mut self, value: Write) {
        // SAFETY: apply holds the captured field lock. Only the selected fields
        // are accessed, so opaque suffix bytes and prefix padding stay untouched.
        unsafe {
            match value {
                Write::Type(value) => ptr::addr_of_mut!((*self.item).bounds_type)
                    .write(item_bounds_setters::valid_bounds_type(value)),
                Write::Alignment(value) => {
                    ptr::addr_of_mut!((*self.item).bounds_align).write(value)
                }
                Write::Crop(value) => ptr::addr_of_mut!((*self.item).crop_to_bounds).write(value),
                Write::Size(value) => {
                    let value = if let Some(bounds) = self.bounds {
                        let value = bounds.read();
                        Vec2::new(value.x, value.y)
                    } else {
                        value
                    };
                    let absolute = ptr::addr_of!((*self.item).absolute_coordinates).read();
                    let height = if absolute {
                        0.0
                    } else {
                        obs_sceneitem_bounds_canvas_height(self.item)
                    };
                    let value = item_bounds_setters::bounds_size(value, absolute, height);
                    ptr::addr_of_mut!((*self.item).bounds).write(vec2 {
                        x: value.x,
                        y: value.y,
                    });
                }
            }
        }
    }
    fn unlock(&mut self) {
        // SAFETY: release precisely the parent captured by lock, including NULL.
        unsafe { obs_sceneitem_fields_unlock(self.scene) };
    }
    fn update(&mut self) {
        // SAFETY: apply released the field lock; C retains the existing dispatch.
        unsafe { obs_sceneitem_bounds_update(self.item) };
    }
}
fn apply(item: *mut SceneItemPrefix, value: Write, bounds: Option<*const vec2>) {
    if !item.is_null() {
        item_bounds_setters::apply(
            Some(&mut Item {
                item,
                scene: ptr::null_mut(),
                bounds,
            }),
            value,
        );
    }
}

/// Validate both setter enums and the full-width integer read by scene loading.
#[unsafe(no_mangle)]
pub extern "C" fn obs_scene_bounds_valid_type(value: i64) -> u32 {
    item_bounds_setters::valid_bounds_type(value)
}

/// Set the bounds type, normalizing unknown values to NONE.
/// # Safety
/// A non-null item and parent must be live under the original C lifecycle contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_set_bounds_type(item: *mut SceneItemPrefix, value: u32) {
    apply(item, Write::Type(i64::from(value)), None);
}
/// Set the bounds alignment, preserving all bits.
/// # Safety
/// A non-null item and parent must remain live for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_set_bounds_alignment(
    item: *mut SceneItemPrefix,
    value: u32,
) {
    apply(item, Write::Alignment(value), None);
}
/// Set whether the item is cropped to its bounds.
/// # Safety
/// A non-null item and parent must remain live for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_set_bounds_crop(item: *mut SceneItemPrefix, value: bool) {
    apply(item, Write::Crop(value), None);
}
/// Set bounds from an absolute size.
/// # Safety
/// For a non-null item, bounds must point to a readable vec2 and the item and
/// parent must remain live. The original API permits NULL bounds only for NULL item.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_set_bounds(item: *mut SceneItemPrefix, bounds: *const vec2) {
    if item.is_null() {
        return;
    }
    apply(item, Write::Size(Vec2::new(0.0, 0.0)), Some(bounds));
}
