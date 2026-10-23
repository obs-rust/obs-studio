//! Safe core of `libobs/obs-scene-crop.c.h`: the scene item crop
//! arithmetic.
//!
//! The scene still reads and writes the item, computes the rescale factors
//! of a nested scene, and decides when any of this runs. These take and
//! return plain values.

/// `obs_scene_cropped_size`.
#[must_use]
pub fn cropped_size(size: u32, crop1: i32, crop2: i32, bounds_crop1: i32, bounds_crop2: i32) -> u32 {
    let _ = (size, crop1, crop2, bounds_crop1, bounds_crop2);
    todo!()
}

/// `obs_scene_scale_crop`.
#[must_use]
pub fn scale_crop(crop: i32, factor: f32) -> i32 {
    let _ = (crop, factor);
    todo!()
}

/// `obs_scene_crop_offset`.
#[must_use]
pub fn crop_offset(crop: i32, bounds_crop: i32) -> f32 {
    let _ = (crop, bounds_crop);
    todo!()
}

/// `obs_scene_clamp_crop`.
#[must_use]
pub fn clamp_crop(crop: i32) -> i32 {
    let _ = crop;
    todo!()
}
