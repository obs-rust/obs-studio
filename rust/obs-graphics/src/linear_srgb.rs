//! The current graphics context's linear-sRGB flag; no context means false.

pub fn linear_srgb(value: Option<bool>) -> bool {
    value.unwrap_or(false)
}

/// Replace the current context's flag, returning its previous value.
/// Without a context, return false and leave all state untouched.
pub fn set_linear_srgb(value: Option<&mut bool>, next: bool) -> bool {
    value.is_some_and(|value| std::mem::replace(value, next))
}
