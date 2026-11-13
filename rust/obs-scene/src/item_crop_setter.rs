//! Store a clamped crop unless the raw input already equals the stored one.
//!
//! Crops are `[left, top, right, bottom]`, the field order of
//! `struct obs_sceneitem_crop`.

use crate::crop::clamp_crop;

/// The original setter checks the item before the crop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Missing {
    Item,
    Crop,
}

/// The null checks of `obs_sceneitem_set_crop`, item first.
pub fn validate(item: bool, crop: bool) -> Result<(), Missing> {
    if !item {
        return Err(Missing::Item);
    }
    if !crop {
        return Err(Missing::Crop);
    }
    Ok(())
}

/// Clamp each side with `obs_scene_clamp_crop`, before the lock is taken.
#[must_use]
pub fn clamp(input: [i32; 4]) -> [i32; 4] {
    input.map(clamp_crop)
}

/// Under the lock: compare the raw input (not the clamped crop) with the
/// stored one. Equal leaves it alone; otherwise the clamped crop is stored.
/// Returns whether it stored, so the caller marks the transform after the
/// unlock.
pub fn store(stored: &mut [i32; 4], input: [i32; 4], clamped: [i32; 4]) -> bool {
    if *stored == input {
        return false;
    }
    *stored = clamped;
    true
}

/// `crop_equal`'s side order, as indexes into `[left, top, right, bottom]`:
/// left, right, top, bottom.
pub const COMPARE_ORDER: [usize; 4] = [0, 2, 1, 3];

/// `crop_equal(input, stored)`: `sides(i)` reads side `i` of the input, then
/// of the stored crop, and is called in [`COMPARE_ORDER`] until the first
/// difference, as C's `&&` chain reads them.
pub fn equal(sides: impl FnMut(usize) -> (i32, i32)) -> bool {
    let _ = sides;
    todo!()
}
