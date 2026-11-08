//! Borrowed canvas fields and video-mix presence; no ownership changes.

/// Copy a borrowed field verbatim, including an absent field.
pub fn borrowed<T: Copy>(field: &T) -> T {
    *field
}

/// A mix is present even when it has no video output yet.
pub fn has_video<T>(mix: Option<T>) -> bool {
    mix.is_some()
}
