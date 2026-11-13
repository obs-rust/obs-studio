//! Store a clamped crop unless the raw input already equals the stored one.
//!
//! Crops are `[left, top, right, bottom]`, the field order of
//! `struct obs_sceneitem_crop`.

/// The original setter checks the item before the crop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Missing {
    Item,
    Crop,
}

/// The null checks of `obs_sceneitem_set_crop`, item first.
pub fn validate(item: bool, crop: bool) -> Result<(), Missing> {
    let _ = (item, crop);
    todo!()
}

/// Clamp each side with `obs_scene_clamp_crop`, before the lock is taken.
#[must_use]
pub fn clamp(input: [i32; 4]) -> [i32; 4] {
    let _ = input;
    todo!()
}

/// Under the lock: compare the raw input (not the clamped crop) with the
/// stored one. Equal leaves it alone; otherwise the clamped crop is stored.
/// Returns whether it stored, so the caller marks the transform after the
/// unlock.
pub fn store(stored: &mut [i32; 4], input: [i32; 4], clamped: [i32; 4]) -> bool {
    let _ = (stored, input, clamped);
    todo!()
}
