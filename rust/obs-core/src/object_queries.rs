//! Read-only context type and privacy queries; NULL has the original defaults.

/// Return the stored object type bits, or INVALID (zero) for no object.
pub fn object_type(value: Option<&i32>) -> i32 {
    let _ = value;
    todo!("port object type query")
}

/// Return the stored privacy flag, or false for no object.
pub fn is_private(value: Option<&bool>) -> bool {
    let _ = value;
    todo!("port object privacy query")
}
