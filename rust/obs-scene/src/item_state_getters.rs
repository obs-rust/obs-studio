//! Read selection and lock flags, preserving null-item defaults.

/// A missing item is not selected.
pub fn selected(value: Option<bool>) -> bool {
    let _ = value;
    todo!()
}

/// A missing item is not locked.
pub fn locked(value: Option<bool>) -> bool {
    let _ = value;
    todo!()
}
