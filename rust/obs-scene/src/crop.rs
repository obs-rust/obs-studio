//! Safe core of `libobs/obs-scene-crop.c.h`: the scene item crop
//! arithmetic.
//!
//! The scene still reads and writes the item, computes the rescale factors
//! of a nested scene (`update_nested_scene_crop`), and decides when any of
//! this runs. These take and return plain values, keep no state and call
//! nothing in libobs.
//!
//! - [`cropped_size`] (`calc_cx` and `calc_cy`): the four crops summed in
//!   64 bits, so they never overflow; 2 when the sum is negative or past
//!   the size, the rest otherwise.
//! - [`scale_crop`] (`update_nested_scene_crop`): `crop as f32 * factor`,
//!   one float multiply rounded to the nearest, then NaN is 0, 2^31 and up
//!   `i32::MAX`, -2^31 and down `i32::MIN`, and the rest truncated toward
//!   0. Rust's saturating `as i32` gives the same values on every input;
//!   the branches are written out to match the C.
//! - [`crop_offset`] (`render_item`): the 64-bit sum as an `f32`, rounded
//!   to the nearest (ties to even), as C's `(float)int64_t` is.
//! - [`clamp_crop`] (`obs_sceneitem_set_crop`): a negative crop is 0.
//!
//! Characterized, not endorsed (`test_scene_item_crop`), and kept: a
//! negative crop that the other crops make up for is subtracted, the crop
//! is a float before the rescale, so it loses its low bits past 2^24, and
//! the truncation makes a rescale and its inverse lossy.
//!
//! No intentional differences.

/// The size left when the crop is more than all of it.
const OVER_CROPPED: u32 = 2;

/// 2^31: the first float past `i32::MAX`. `INT_MAX as f32` rounds to it.
const I32_END: f32 = 2_147_483_648.0;

/// `obs_scene_cropped_size`: the size of a source of `size` pixels after
/// the crops on both sides (the item's and the bounds crop), or 2 when they
/// crop more than all of it or sum to less than 0.
#[must_use]
pub fn cropped_size(
    size: u32,
    crop1: i32,
    crop2: i32,
    bounds_crop1: i32,
    bounds_crop2: i32,
) -> u32 {
    let crop =
        i64::from(crop1) + i64::from(crop2) + i64::from(bounds_crop1) + i64::from(bounds_crop2);
    // A negative sum or one past u32::MAX does not convert; one past the
    // size fails the guard.
    match u32::try_from(crop) {
        Ok(crop) if crop <= size => size - crop,
        _ => OVER_CROPPED,
    }
}

/// `obs_scene_scale_crop`: `crop` scaled by `factor`, truncated toward 0;
/// the nearest `i32` past the range, 0 for NaN.
#[must_use]
pub fn scale_crop(crop: i32, factor: f32) -> i32 {
    // C's (float)crop: the nearest float, ties to even.
    let scaled = crop as f32 * factor;
    if scaled.is_nan() {
        return 0;
    }
    if scaled >= I32_END {
        return i32::MAX;
    }
    if scaled <= -I32_END {
        return i32::MIN;
    }
    // In (-2^31, 2^31), so the truncation toward 0 fits.
    scaled as i32
}

/// `obs_scene_crop_offset`: how far the crops move the source when it is
/// drawn, `crop + bounds_crop` in 64 bits as the nearest float.
#[must_use]
pub fn crop_offset(crop: i32, bounds_crop: i32) -> f32 {
    // C's (float)int64_t: the nearest float, ties to even.
    (i64::from(crop) + i64::from(bounds_crop)) as f32
}

/// `obs_scene_clamp_crop`: a negative crop is 0; any other is kept.
#[must_use]
pub fn clamp_crop(crop: i32) -> i32 {
    if crop < 0 { 0 } else { crop }
}
