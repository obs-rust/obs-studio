//! Borrowed parent selection for a scene item.

/// Return the stored parent, or the null/default value for an absent item.
pub fn parent<T: Default>(value: Option<T>) -> T {
    let _ = value;
    todo!("port obs_sceneitem_get_scene")
}
