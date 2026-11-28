//! Thin pointer access over the already measured scene item prefix.
use super::item_bounds_getters::SceneItemPrefix;
use crate::{
    item_bounds_setters::Fields,
    item_xform_setters::{self, Item, Value},
};
use core::{ffi::c_void, ptr};
use obs_graphics::ffi::vec2::vec2;

unsafe extern "C" {
    fn obs_sceneitem_fields_lock(item: *const SceneItemPrefix) -> *mut c_void;
    fn obs_sceneitem_fields_unlock(parent: *mut c_void);
    fn obs_sceneitem_bounds_update_transform(item: *mut SceneItemPrefix);
    fn obs_sceneitem_xform_pos_from_absolute(
        dst: *mut vec2,
        value: *const vec2,
        item: *const SceneItemPrefix,
    );
    fn obs_sceneitem_xform_relative_scale(
        dst: *mut vec2,
        value: *const vec2,
        item: *const SceneItemPrefix,
    );
}

enum Input {
    Position(*const vec2),
    Rotation(f32),
    Scale(*const vec2),
}
struct Live {
    item: *mut SceneItemPrefix,
    parent: *mut c_void,
    input: Input,
}
impl Fields for Live {
    fn lock(&mut self) {
        // SAFETY: constructed only for a live item under the C caller contract.
        self.parent = unsafe { obs_sceneitem_fields_lock(self.item) };
    }
    fn unlock(&mut self) {
        // SAFETY: precisely the parent captured by lock, including NULL.
        unsafe { obs_sceneitem_fields_unlock(self.parent) };
    }
    fn update(&mut self) {
        // SAFETY: live item after unlocking, using the existing C dispatch.
        unsafe { obs_sceneitem_bounds_update_transform(self.item) };
    }
}
impl Item for Live {
    fn input(&self) -> Value {
        // SAFETY: read input under the fields lock. C requires a readable vector
        // for live items; it may alias either stored vector.
        unsafe {
            match self.input {
                Input::Position(v) => Value::Position([(*v).x, (*v).y]),
                Input::Rotation(v) => Value::Rotation(v),
                Input::Scale(v) => Value::Scale([(*v).x, (*v).y]),
            }
        }
    }
    fn absolute(&self) -> bool {
        // SAFETY: only this initialized field is read, under the lock.
        unsafe { ptr::addr_of!((*self.item).absolute_coordinates).read() }
    }
    fn relative(&mut self, value: Value) -> Value {
        let mut out = vec2::default();
        // SAFETY: local input/output and live locked item. These existing scene
        // conversions call the Rust coordinate helpers and preserve canvas reads.
        unsafe {
            match value {
                Value::Position([x, y]) => {
                    obs_sceneitem_xform_pos_from_absolute(&raw mut out, &vec2 { x, y }, self.item);
                    Value::Position([out.x, out.y])
                }
                Value::Scale([x, y]) => {
                    obs_sceneitem_xform_relative_scale(&raw mut out, &vec2 { x, y }, self.item);
                    Value::Scale([out.x, out.y])
                }
                Value::Rotation(_) => value,
            }
        }
    }
    fn store(&mut self, value: Value) {
        // SAFETY: write only the chosen field under the fields lock; never form
        // a reference to the prefix or read its possibly uninitialized suffix.
        unsafe {
            match value {
                Value::Position([x, y]) => ptr::addr_of_mut!((*self.item).pos).write(vec2 { x, y }),
                Value::Rotation(v) => ptr::addr_of_mut!((*self.item).rot).write(v),
                Value::Scale([x, y]) => ptr::addr_of_mut!((*self.item).scale).write(vec2 { x, y }),
            }
        }
    }
}
fn apply(item: *mut SceneItemPrefix, input: Input) {
    let mut live = Live {
        item,
        parent: ptr::null_mut(),
        input,
    };
    item_xform_setters::set((!item.is_null()).then_some(&mut live));
}
/// Set position, silently ignoring a NULL item.
/// # Safety
/// A live item and its parent stay valid; `pos` is readable and may alias item fields.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_set_pos(item: *mut SceneItemPrefix, pos: *const vec2) {
    apply(item, Input::Position(pos));
}
/// Set unnormalized rotation, silently ignoring a NULL item.
/// # Safety
/// A non-null item and its parent stay live for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_set_rot(item: *mut SceneItemPrefix, rot: f32) {
    apply(item, Input::Rotation(rot));
}
/// Set scale, silently ignoring a NULL item.
/// # Safety
/// A live item and its parent stay valid; `scale` is readable and may alias item fields.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_set_scale(item: *mut SceneItemPrefix, scale: *const vec2) {
    apply(item, Input::Scale(scale));
}
