//! Reconnect callback and data are replaced together; clearing discards data.

/// Select the stored callback and data without invoking the callback.
pub fn select<C, D: Default>(callback: Option<C>, data: D) -> (Option<C>, D) {
    if callback.is_some() {
        (callback, data)
    } else {
        (None, D::default())
    }
}
