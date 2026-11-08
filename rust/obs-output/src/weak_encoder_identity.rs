//! Weak encoder identity without dereferencing encoder handles.
pub fn references(stored: Option<usize>, encoder: usize) -> bool {
    encoder != 0 && stored.is_some_and(|stored| stored == encoder)
}
