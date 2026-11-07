//! Borrowed global field reads. Ownership and initialization remain with C.

/// Read a borrowed field without allocating, retaining, or inspecting its value.
pub fn borrowed<T: Copy>(field: &T) -> T {
    *field
}
