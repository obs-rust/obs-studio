//! C ABI for `libobs/obs-scene-crop.c.h`, declared in
//! `libobs/obs-scene-crop.h` without `EXPORT`.
//!
//! `obs-scene.c.h` passes the item's crop and bounds crop fields, the
//! source size and the rescale factor by value, and stores what comes back,
//! so `struct obs_scene_item` and `struct obs_sceneitem_crop` are never
//! mirrored. No pointers. The symbols stay hidden (`rust-hidden.txt`).

use std::ffi::c_int;

use crate::crop as core;

/// Same contract as C `obs_scene_cropped_size`. No pointers.
#[unsafe(no_mangle)]
pub extern "C" fn obs_scene_cropped_size(
    size: u32,
    crop1: c_int,
    crop2: c_int,
    bounds_crop1: c_int,
    bounds_crop2: c_int,
) -> u32 {
    core::cropped_size(size, crop1, crop2, bounds_crop1, bounds_crop2)
}

/// Same contract as C `obs_scene_scale_crop`. No pointers.
#[unsafe(no_mangle)]
pub extern "C" fn obs_scene_scale_crop(crop: c_int, factor: f32) -> c_int {
    core::scale_crop(crop, factor)
}

/// Same contract as C `obs_scene_crop_offset`. No pointers.
#[unsafe(no_mangle)]
pub extern "C" fn obs_scene_crop_offset(crop: c_int, bounds_crop: c_int) -> f32 {
    core::crop_offset(crop, bounds_crop)
}

/// Same contract as C `obs_scene_clamp_crop`. No pointers.
#[unsafe(no_mangle)]
pub extern "C" fn obs_scene_clamp_crop(crop: c_int) -> c_int {
    core::clamp_crop(crop)
}
