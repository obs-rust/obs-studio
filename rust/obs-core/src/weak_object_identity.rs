//! Weak object identity without dereferencing object handles.
pub fn references(stored: Option<usize>, object: usize) -> bool {
    object != 0 && stored.is_some_and(|stored| stored == object)
}
