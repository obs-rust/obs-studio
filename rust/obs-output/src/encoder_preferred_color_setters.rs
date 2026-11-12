//! Preferred color mutation independent of the private encoder layout.

/// Preserve absent/non-video no-ops and store video enum bits unchanged.
pub fn set(_fields: Option<(i32, &mut u32)>, _value: u32) {
    todo!()
}
