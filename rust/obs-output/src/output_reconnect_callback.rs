//! Reconnect callback and data are replaced together; clearing discards data.

/// Select the stored callback and data without invoking the callback.
pub fn select<C, D: Default>(callback: Option<C>, data: D) -> (Option<C>, D) {
    if callback.is_some() {
        (callback, data)
    } else {
        (None, D::default())
    }
}

/// The setter needs a live output. A NULL output is refused as
/// `obs_output_valid` refuses it, where the C wrote through the NULL
/// output (#4754, the one intentional difference).
pub fn accepts_output(output: bool) -> bool {
    let _ = output;
    todo!()
}
