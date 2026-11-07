//! Stored transform scalars and their null-item defaults.

/// Read the stored rotation, preserving every float bit; NULL returns positive zero.
pub fn rotation(value: Option<f32>) -> f32 {
    todo!("#3897: {value:?}")
}

/// Read the stored alignment; NULL returns zero.
pub fn alignment(value: Option<u32>) -> u32 {
    todo!("#3897: {value:?}")
}
