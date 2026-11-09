//! Reconnect callback and data are replaced together; clearing discards data.

/// Select the stored callback and data without invoking the callback.
pub fn select<C, D: Default>(callback: Option<C>, data: D) -> (Option<C>, D) {
    let _ = (callback, data);
    todo!("port obs_output_set_reconnect_callback")
}
