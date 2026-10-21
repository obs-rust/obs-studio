//! C ABI for `libobs/obs-scene-bounds.c.h`.
//!
//! The shim reads both pointers, then writes `scale`. A null `bounds` or
//! `scale` is not checked. `obs-scene.c` calls this inside libobs. The C
//! file has no `EXPORT`, so the symbol stays hidden.

use std::ffi::c_int;
use std::ptr;

use obs_graphics::ffi::vec2::vec2;
use obs_graphics::vec2::Vec2;

use crate::bounds::{BoundsType, bounds_fit_scale};

/// Same contract as C `obs_bounds_fit_scale`.
///
/// # Safety
///
/// `bounds` and `scale` must be readable, and `scale` writable. They are
/// not the same pointer in `obs-scene.c`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_bounds_fit_scale(
    type_: c_int,
    bounds: *const vec2,
    cx: u32,
    cy: u32,
    scale: *mut vec2,
) {
    // SAFETY: the caller guarantees both pointers are readable.
    let (box_, src) = unsafe { (ptr::read(bounds), ptr::read(scale)) };
    let out = bounds_fit_scale(
        BoundsType::from_c(type_),
        Vec2::from(box_),
        cx,
        cy,
        Vec2::from(src),
    );
    // SAFETY: the caller guarantees `scale` is writable.
    unsafe { ptr::write(scale, out.into()) };
}
