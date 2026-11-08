//! Weak encoder identity without dereferencing encoder handles.
pub fn references(stored: Option<usize>, encoder: usize) -> bool {
    let _ = (stored, encoder);
    todo!("weak encoder identity")
}
