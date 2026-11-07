//! Borrowed canvas fields and video-mix presence; no ownership changes.

/// Copy a borrowed field verbatim, including an absent field.
pub fn borrowed<T: Copy>(_field: &T) -> T {
    todo!("#3860")
}

/// A mix is present even when it has no video output yet.
pub fn has_video<T>(_mix: Option<T>) -> bool {
    todo!("#3860")
}
