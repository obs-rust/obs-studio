//! Store blending methods verbatim, without dirtying the transform.

/// Absent items report the original validation failure without accessing fields.
pub fn set_method(field: Option<&mut u32>, method: u32, invalid: impl FnOnce()) {
    let _ = (field, method, invalid);
    todo!()
}
