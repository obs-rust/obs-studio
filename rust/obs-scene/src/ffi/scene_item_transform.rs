//! C ABI for `libobs/obs-scene-item-transform.c.h`.
//!
//! `compute_item_transform()` in `obs-scene.c.h` calls these four steps
//! with pointers into the item (its crop, bounds crop, last size and
//! matrices) and its own locals (the scale, origins and position). Each
//! shim reads all its inputs, then writes its outputs. A null pointer is
//! not checked. The C file has no `EXPORT`, so the symbols stay hidden
//! (`rust-hidden.txt`, `rust-exports.map`, `rust-unexports-macos.txt`).
//!
//! No float becomes an integer here: the six casts stay in `obs-scene.c.h`.

use core::ffi::c_int;
use core::ptr;

use obs_graphics::ffi::matrix4::matrix4;
use obs_graphics::ffi::vec2::vec2;
use obs_graphics::vec2::Vec2;

use crate::ffi::scene_bounds_place::SceneItemCrop;
use crate::item_transform;

/// `struct obs_scene_item_transform_size`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub struct obs_scene_item_transform_size {
    pub cx: u32,
    pub cy: u32,
}

/// Same contract as C `obs_scene_item_transform_prepare`.
///
/// # Safety
///
/// `crop`, `bounds_crop`, `last_width` and `last_height` must be readable
/// and writable. `obs-scene.c.h` passes four distinct fields of one item.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_scene_item_transform_prepare(
    crop: *mut SceneItemCrop,
    bounds_crop: *mut SceneItemCrop,
    last_width: *mut u32,
    last_height: *mut u32,
    width: u32,
    height: u32,
    nested: bool,
) -> obs_scene_item_transform_size {
    // SAFETY: the caller guarantees the pointers are readable.
    let (crop_in, last_width_in, last_height_in) = unsafe {
        (
            ptr::read(crop),
            ptr::read(last_width),
            ptr::read(last_height),
        )
    };
    let out = item_transform::prepare(
        crop_in.into(),
        last_width_in,
        last_height_in,
        width,
        height,
        nested,
    );
    // SAFETY: the caller guarantees the four outputs are writable.
    unsafe {
        ptr::write(crop, out.crop.into());
        ptr::write(bounds_crop, out.bounds_crop.into());
        ptr::write(last_width, out.last_width);
        ptr::write(last_height, out.last_height);
    }
    obs_scene_item_transform_size {
        cx: out.cx,
        cy: out.cy,
    }
}

/// Same contract as C `obs_scene_item_transform_bounded`. `bounds_type`
/// is the C's `int` value of `enum obs_bounds_type`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_scene_item_transform_bounded(bounds_type: c_int) -> bool {
    item_transform::bounded(bounds_type)
}

/// Same contract as C `obs_scene_item_transform_matrix`.
///
/// # Safety
///
/// `scale`, `origin` and `position` must be readable. `dst` must be
/// writable. `obs-scene.c.h` passes an item's matrix and its own locals.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_scene_item_transform_matrix(
    dst: *mut matrix4,
    scale: *const vec2,
    origin: *const vec2,
    rot: f32,
    position: *const vec2,
) {
    // SAFETY: the caller guarantees the inputs are readable.
    let (scale, origin, position) =
        unsafe { (ptr::read(scale), ptr::read(origin), ptr::read(position)) };
    let out = item_transform::matrix(
        Vec2::from(scale),
        Vec2::from(origin),
        rot,
        Vec2::from(position),
    );
    // SAFETY: the caller guarantees `dst` is writable.
    unsafe { ptr::write(dst, out.into()) };
}

/// Same contract as C `obs_scene_item_transform_box_scale`.
///
/// # Safety
///
/// `scale` must be readable and writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_scene_item_transform_box_scale(
    scale: *mut vec2,
    width: u32,
    height: u32,
) {
    // SAFETY: the caller guarantees `scale` is readable.
    let v = unsafe { ptr::read(scale) };
    let out = item_transform::box_scale(Vec2::from(v), width, height);
    // SAFETY: the caller guarantees `scale` is writable.
    unsafe { ptr::write(scale, out.into()) };
}
