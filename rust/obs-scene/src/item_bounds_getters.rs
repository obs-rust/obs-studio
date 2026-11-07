//! Stored scalar bounds fields and their null-item defaults.

/// Read a stored bounds type; a missing item has no bounds.
pub fn bounds_type(value: Option<u32>) -> u32 {
    value.unwrap_or(0)
}

/// Read a stored bounds alignment; a missing item returns zero.
pub fn bounds_alignment(value: Option<u32>) -> u32 {
    value.unwrap_or(0)
}

/// Read the stored crop flag; a missing item returns false.
pub fn bounds_crop(value: Option<bool>) -> bool {
    value.unwrap_or(false)
}
