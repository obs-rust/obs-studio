//! Stored transform scalars and their null-item defaults.

/// Read the stored rotation, preserving every float bit; NULL returns positive zero.
pub fn rotation(value: Option<f32>) -> f32 {
    value.unwrap_or(0.0)
}

/// Read the stored alignment; NULL returns zero.
pub fn alignment(value: Option<u32>) -> u32 {
    value.unwrap_or(0)
}
