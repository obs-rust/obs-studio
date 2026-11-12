//! Preferred color mutation independent of the private encoder layout.

/// Preserve absent/non-video no-ops and store video enum bits unchanged.
pub fn set(fields: Option<(i32, &mut u32)>, value: u32) {
    if let Some((1, stored)) = fields {
        *stored = value;
    }
}
