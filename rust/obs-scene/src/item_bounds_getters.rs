//! Stored scalar bounds fields and their null-item defaults.

/// Read a stored bounds type; a missing item has no bounds.
pub fn bounds_type(_value: Option<u32>) -> u32 {
    todo!("#3897")
}

/// Read a stored bounds alignment; a missing item returns zero.
pub fn bounds_alignment(_value: Option<u32>) -> u32 {
    todo!("#3897")
}

/// Read the stored crop flag; a missing item returns false.
pub fn bounds_crop(_value: Option<bool>) -> bool {
    todo!("#3897")
}
