//! `oracle_*` build of `libobs/obs-scene-crop.c.h` (oracle/scene_crop.c).

use std::ffi::c_int;

unsafe extern "C" {
    pub fn oracle_obs_scene_cropped_size(
        size: u32,
        crop1: c_int,
        crop2: c_int,
        bounds_crop1: c_int,
        bounds_crop2: c_int,
    ) -> u32;

    pub fn oracle_obs_scene_scale_crop(crop: c_int, factor: f32) -> c_int;

    pub fn oracle_obs_scene_crop_offset(crop: c_int, bounds_crop: c_int) -> f32;

    pub fn oracle_obs_scene_clamp_crop(crop: c_int) -> c_int;
}
