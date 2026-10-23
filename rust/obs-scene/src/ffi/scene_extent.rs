//! C ABI for `libobs/obs-scene-extent.c.h`, declared in
//! `libobs/obs-scene-extent.h` without `EXPORT`.
//!
//! `resize_scene_base` in `obs-scene.c.h` passes its own `minv`, `maxv` and
//! `scale`, the scene's `cx` and `cy`, and `&item->box_transform` for each
//! item, so `struct obs_scene_item` is never mirrored. Each shim reads its
//! inputs, then writes its outputs. A null pointer is not checked. The
//! symbols stay hidden (`rust-hidden.txt`).

use std::ptr;

use obs_graphics::ffi::matrix4::matrix4;
use obs_graphics::ffi::vec2::vec2;
use obs_graphics::matrix4::Matrix4;
use obs_graphics::vec2::Vec2;

use crate::extent::{self as core, Extent};

/// Same contract as C `obs_scene_extent_init`.
///
/// # Safety
///
/// `minv` and `maxv` must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_scene_extent_init(minv: *mut vec2, maxv: *mut vec2) {
    let e = core::init();
    // SAFETY: the caller guarantees both are writable.
    unsafe {
        ptr::write(minv, e.min.into());
        ptr::write(maxv, e.max.into());
    }
}

/// Same contract as C `obs_scene_extent_add`.
///
/// # Safety
///
/// `minv` and `maxv` must be readable and writable, and `box_transform`
/// readable. `obs-scene.c` passes two distinct vectors and the item's
/// matrix.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_scene_extent_add(
    minv: *mut vec2,
    maxv: *mut vec2,
    box_transform: *const matrix4,
) {
    // SAFETY: the caller guarantees every pointer is readable.
    let (min, max, m) = unsafe { (ptr::read(minv), ptr::read(maxv), ptr::read(box_transform)) };
    let mut e = Extent {
        min: Vec2::from(min),
        max: Vec2::from(max),
    };
    core::add(&mut e, &Matrix4::from(m));
    // SAFETY: the caller guarantees both are writable.
    unsafe {
        ptr::write(minv, e.min.into());
        ptr::write(maxv, e.max.into());
    }
}

/// Same contract as C `obs_scene_extent_size`.
///
/// # Safety
///
/// `minv` and `maxv` must be readable, and `scale`, `cx` and `cy` writable.
/// `obs-scene.c` passes distinct objects.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_scene_extent_size(
    scale: *mut vec2,
    cx: *mut u32,
    cy: *mut u32,
    minv: *const vec2,
    maxv: *const vec2,
) {
    // SAFETY: the caller guarantees both are readable.
    let (min, max) = unsafe { (ptr::read(minv), ptr::read(maxv)) };
    let s = core::size(&Extent {
        min: Vec2::from(min),
        max: Vec2::from(max),
    });
    // SAFETY: the caller guarantees the three outputs are writable.
    unsafe {
        ptr::write(scale, s.scale.into());
        ptr::write(cx, s.cx);
        ptr::write(cy, s.cy);
    }
}
