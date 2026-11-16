//! Borrow a group's scene without reading a non-group's source.

/// Invoke the scene reader only for a present group item.
pub fn group_scene<T: Default>(is_group: Option<bool>, read_scene: impl FnOnce() -> T) -> T {
    if is_group == Some(true) {
        read_scene()
    } else {
        T::default()
    }
}
