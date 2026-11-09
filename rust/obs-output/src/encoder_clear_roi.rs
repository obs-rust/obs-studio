//! Clear ROI state while retaining its allocation and original lock ordering.

/// Access to the same state and mutex used by encoder ROI add/enumeration.
pub trait RoiState {
    fn count(&self) -> usize;
    fn with_locked_fields(&mut self, update: impl FnOnce(&mut usize, &mut u32));
}

/// NULL is diagnosed; an empty pre-lock count is a no-op.
pub fn clear_roi(state: Option<&mut impl RoiState>, invalid: impl FnOnce()) {
    let _ = (state, invalid);
    todo!()
}
