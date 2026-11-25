//! Bit-preserving stored-field access using the existing C parent mutex.
use super::item_bounds_getters::SceneItemPrefix;
use crate::item_matrix_getters::{FieldAccess, copy_field};
use core::{ffi::c_void, ptr};
use obs_graphics::ffi::{matrix4::matrix4, vec2::vec2};

unsafe extern "C" {
    fn obs_sceneitem_fields_lock(item: *const SceneItemPrefix) -> *mut c_void;
    fn obs_sceneitem_fields_unlock(scene: *mut c_void);
}

// Constructed only for a live item, initialized field and writable output by
// the unsafe entry points below. Raw pointers preserve C's output aliasing.
struct Access<const N: usize> {
    item: *const SceneItemPrefix,
    field: *const [u32; N],
    output: *mut [u32; N],
}

impl<const N: usize> FieldAccess<N> for Access<N> {
    type Parent = *mut c_void;
    fn lock(&mut self) -> Self::Parent {
        // SAFETY: the caller keeps the item and captured parent live.
        unsafe { obs_sceneitem_fields_lock(self.item) }
    }
    fn read(&self) -> [u32; N] {
        // SAFETY: initialized matrix/vec2 storage has the same size as these
        // integer cells and sufficient alignment; all bits are valid, under lock.
        unsafe { self.field.read() }
    }
    fn write(&mut self, bits: [u32; N]) {
        // SAFETY: output is writable for exactly this field, including aliasing
        // its storage. The entire input was read before this write.
        unsafe { self.output.write(bits) };
    }
    fn unlock(&mut self, parent: Self::Parent) {
        // SAFETY: this is precisely the parent returned by lock, possibly NULL.
        unsafe { obs_sceneitem_fields_unlock(parent) };
    }
}

/// Copy the stored draw matrix. A NULL item leaves output untouched.
///
/// # Safety
/// A live item and its parent must remain valid throughout the call. For a
/// non-NULL item, output must be writable matrix4 storage, as in the C API.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_get_draw_transform(
    item: *const SceneItemPrefix,
    output: *mut matrix4,
) {
    if item.is_null() {
        return;
    }
    // SAFETY: forming a field address does not read it; item is live.
    let field = unsafe { ptr::addr_of!((*item).draw_transform) }.cast();
    copy_field(Some(&mut Access::<16> {
        item,
        field,
        output: output.cast(),
    }));
}

/// Copy the stored box matrix. A NULL item leaves output untouched.
///
/// # Safety
/// Same lifecycle and writable-output contract as get_draw_transform.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_get_box_transform(
    item: *const SceneItemPrefix,
    output: *mut matrix4,
) {
    if item.is_null() {
        return;
    }
    // SAFETY: forming a field address does not read it; item is live.
    let field = unsafe { ptr::addr_of!((*item).box_transform) }.cast();
    copy_field(Some(&mut Access::<16> {
        item,
        field,
        output: output.cast(),
    }));
}

/// Copy the stored box scale. A NULL item leaves output untouched.
///
/// # Safety
/// A live item and parent must remain valid; its output must be writable vec2
/// storage. A NULL output is permitted only when item is NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_get_box_scale(
    item: *const SceneItemPrefix,
    output: *mut vec2,
) {
    if item.is_null() {
        return;
    }
    // SAFETY: forming a field address does not read it; item is live.
    let field = unsafe { ptr::addr_of!((*item).box_scale) }.cast();
    copy_field(Some(&mut Access::<2> {
        item,
        field,
        output: output.cast(),
    }));
}
