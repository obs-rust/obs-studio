//! Store a scene item's blend mode and invalidate its transform.

/// Preserve arbitrary stored enum values and always dirty a present item.
pub fn set(mode: Option<&mut u32>, value: u32, dirty: impl FnOnce(), null: impl FnOnce()) {
    let _ = (mode, value, dirty, null);
    todo!("scene item blending mode setter")
}
