//! Stored scene and item group membership flags.

/// Return the stored group flag, or false for an absent scene or item.
pub fn is_group(value: Option<&bool>) -> bool {
    value.copied().unwrap_or(false)
}
