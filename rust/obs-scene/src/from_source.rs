//! Borrow scene context only when a source has the requested exact identifier.

/// Missing and mismatched sources never read their context or acquire ownership.
pub fn from_source<T: Default>(
    id: Option<&[u8]>,
    expected: &[u8],
    read_context: impl FnOnce() -> T,
) -> T {
    if id == Some(expected) {
        read_context()
    } else {
        T::default()
    }
}
