//! Snapshot the stored argument count and borrowed argument table.

/// Copy the two stored values without inspecting or modifying the argument table.
pub fn snapshot<T: Copy>(argc: &i32, argv: &T) -> (i32, T) {
    (*argc, *argv)
}
