//! Decision for the scene-item lock setter, independent of C object layout.

/// Return the value to store and signal, or no change for a missing/detached item.
pub fn set_locked(current: Option<bool>, has_parent: bool, requested: bool) -> Option<bool> {
    let _ = (current, has_parent, requested);
    todo!("port obs_sceneitem_set_locked")
}
