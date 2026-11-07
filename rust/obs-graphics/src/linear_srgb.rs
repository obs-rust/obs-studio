//! The current graphics context's linear-sRGB flag; no context means false.

pub fn linear_srgb(value: Option<bool>) -> bool {
    value.unwrap_or(false)
}
