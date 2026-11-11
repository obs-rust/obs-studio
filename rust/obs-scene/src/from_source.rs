//! Borrow scene context only when a source has the requested exact identifier.

/// Missing and mismatched sources never read their context or acquire ownership.
pub fn from_source<T: Default>(
    _id: Option<&[u8]>,
    _expected: &[u8],
    _read_context: impl FnOnce() -> T,
) -> T {
    todo!("port the borrowed scene/source conversion")
}
