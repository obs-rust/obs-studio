//! Snapshot the stored argument count and borrowed argument table.

/// Copy the two stored values without inspecting or modifying the argument table.
pub fn snapshot<T: Copy>(_argc: &i32, _argv: &T) -> (i32, T) {
    todo!("port the command-line snapshot getter")
}
