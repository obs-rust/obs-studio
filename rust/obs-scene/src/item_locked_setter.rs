//! Decision for the scene-item lock setter, independent of C object layout.

/// Return the value to store and signal, or no change for a missing/detached item.
pub fn set_locked(current: Option<bool>, has_parent: bool, requested: bool) -> Option<bool> {
    current
        .filter(|&value| value != requested && has_parent)
        .map(|_| requested)
}
