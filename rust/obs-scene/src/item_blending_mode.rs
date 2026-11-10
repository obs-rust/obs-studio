//! Store a scene item's blend mode and invalidate its transform.

/// Preserve arbitrary stored enum values and always dirty a present item.
pub fn set(mode: Option<&mut u32>, value: u32, dirty: impl FnOnce(), null: impl FnOnce()) {
    if let Some(mode) = mode {
        *mode = value;
        dirty();
    } else {
        null();
    }
}
