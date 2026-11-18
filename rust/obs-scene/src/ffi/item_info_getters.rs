//! Synchronized position, bounds and transform-info reads through the
//! existing captured-parent C lock and canvas conversion boundaries.
use super::{item_bounds_getters::SceneItemPrefix, transform_states::TransformInfo};
use crate::item_info_getters::{Conversion, TransformBits, read_info, read_vec2};
use core::{ffi::c_void, ptr};
use obs_graphics::ffi::vec2::vec2;

unsafe extern "C" {
    fn obs_sceneitem_fields_lock(item: *const SceneItemPrefix) -> *mut c_void;
    fn obs_sceneitem_fields_unlock(scene: *mut c_void);
    fn obs_sceneitem_pos_canvas_read(
        output: *mut vec2,
        value: *const vec2,
        item: *const SceneItemPrefix,
    );
    fn obs_sceneitem_size_canvas_read(
        output: *mut vec2,
        value: *const vec2,
        item: *const SceneItemPrefix,
    );
    fn obs_sceneitem_scale_canvas_read(output: *mut vec2, item: *const SceneItemPrefix);
}

fn bits(value: vec2) -> [u32; 2] {
    [value.x.to_bits(), value.y.to_bits()]
}

fn floats(bits: [u32; 2]) -> [f32; 2] {
    [f32::from_bits(bits[0]), f32::from_bits(bits[1])]
}

/// Run one C canvas conversion of a stored field.
///
/// # Safety
/// The item is live and its captured parent's video mutex is held.
unsafe fn convert(item: *const SceneItemPrefix, conversion: Conversion) -> [u32; 2] {
    let mut out = vec2::default();
    // SAFETY: live C item, writable local output, and the field pointers point
    // into that item, all under the held mutex, as the original C passed them.
    unsafe {
        match conversion {
            Conversion::Position => {
                obs_sceneitem_pos_canvas_read(&raw mut out, ptr::addr_of!((*item).pos), item);
            }
            Conversion::Scale => obs_sceneitem_scale_canvas_read(&raw mut out, item),
            Conversion::Bounds => {
                obs_sceneitem_size_canvas_read(&raw mut out, ptr::addr_of!((*item).bounds), item);
            }
        }
    }
    bits(out)
}

/// One stored vec2 field, the stored value or its C conversion.
///
/// # Safety
/// As the exported getters: a live item and parent, writable output.
unsafe fn read_field(
    item: *const SceneItemPrefix,
    output: *mut vec2,
    field: *const vec2,
    conversion: Conversion,
) {
    // SAFETY: C captures the live parent and takes exactly its video mutex.
    let scene = unsafe { obs_sceneitem_fields_lock(item) };
    // SAFETY: read only the required fields under the captured parent's lock.
    let (stored, absolute) = unsafe {
        (
            field.read(),
            ptr::addr_of!((*item).absolute_coordinates).read(),
        )
    };
    let mut out = [0; 2];
    read_vec2(
        Some((bits(stored), absolute)),
        // SAFETY: the item is live and its parent's video mutex is held.
        || unsafe { convert(item, conversion) },
        &mut out,
    );
    let [x, y] = floats(out);
    // SAFETY: the caller supplies output storage; every input was read before
    // the write, so an output aliasing the stored field behaves as in C.
    unsafe {
        output.write(vec2 { x, y });
        obs_sceneitem_fields_unlock(scene);
    }
}

/// Read the position, converting relative coordinates in C.
/// A NULL item leaves the output untouched, including a NULL output.
///
/// # Safety
/// A non-null item and its captured parent must remain live as in the original
/// C getter. Its output must point to writable vec2 storage and may alias pos.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_get_pos(item: *const SceneItemPrefix, output: *mut vec2) {
    if item.is_null() {
        return;
    }
    // SAFETY: live item and writable output under the caller's contract.
    unsafe {
        read_field(
            item,
            output,
            ptr::addr_of!((*item).pos),
            Conversion::Position,
        )
    }
}

/// Read the bounds, converting relative coordinates in C.
/// A NULL item leaves the output untouched, including a NULL output.
///
/// # Safety
/// As [`obs_sceneitem_get_pos`]; the output may alias the stored bounds.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_get_bounds(item: *const SceneItemPrefix, output: *mut vec2) {
    if item.is_null() {
        return;
    }
    // SAFETY: live item and writable output under the caller's contract.
    unsafe {
        read_field(
            item,
            output,
            ptr::addr_of!((*item).bounds),
            Conversion::Bounds,
        )
    }
}

/// Fill `struct obs_transform_info` field by field, so its padding keeps
/// whatever the caller left there. A NULL item or info writes nothing and
/// takes no lock.
///
/// # Safety
/// A non-null item and its captured parent must remain live as in the original
/// C getter; a non-null info must point to writable `obs_transform_info`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_get_info2(
    item: *const SceneItemPrefix,
    info: *mut TransformInfo,
) {
    if item.is_null() || info.is_null() {
        return;
    }
    // SAFETY: C captures the live parent and takes exactly its video mutex.
    let scene = unsafe { obs_sceneitem_fields_lock(item) };
    // SAFETY: read only the info fields under the captured parent's lock.
    let (stored, absolute) = unsafe {
        (
            TransformBits {
                pos: bits(ptr::addr_of!((*item).pos).read()),
                rot: ptr::addr_of!((*item).rot).read().to_bits(),
                scale: bits(ptr::addr_of!((*item).scale).read()),
                alignment: ptr::addr_of!((*item).align).read(),
                bounds_type: ptr::addr_of!((*item).bounds_type).read(),
                bounds_alignment: ptr::addr_of!((*item).bounds_align).read(),
                bounds: bits(ptr::addr_of!((*item).bounds).read()),
                crop_to_bounds: ptr::addr_of!((*item).crop_to_bounds).read(),
            },
            ptr::addr_of!((*item).absolute_coordinates).read(),
        )
    };
    // SAFETY: the item is live and its parent's video mutex is held.
    let out = read_info(Some((stored, absolute)), |conversion| unsafe {
        convert(item, conversion)
    });
    // SAFETY: the caller supplies writable info storage; each field is written
    // alone, in the C order, and the padding is never touched. The same
    // captured scene is unlocked afterwards.
    unsafe {
        if let Some(out) = out {
            ptr::addr_of_mut!((*info).pos).write(floats(out.pos));
            ptr::addr_of_mut!((*info).scale).write(floats(out.scale));
            ptr::addr_of_mut!((*info).bounds).write(floats(out.bounds));
            ptr::addr_of_mut!((*info).rot).write(f32::from_bits(out.rot));
            ptr::addr_of_mut!((*info).alignment).write(out.alignment);
            ptr::addr_of_mut!((*info).bounds_type).write(out.bounds_type);
            ptr::addr_of_mut!((*info).bounds_alignment).write(out.bounds_alignment);
            ptr::addr_of_mut!((*info).crop_to_bounds).write(out.crop_to_bounds);
        }
        obs_sceneitem_fields_unlock(scene);
    }
}
