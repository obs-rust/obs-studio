//! Forward filter serialization, with storage and locking supplied by the host.

/// Operations on a live source and its owned serialization objects.
pub trait Backend {
    /// An independently owned backup array.
    type Array;
    /// A temporary reference to a saved filter.
    type Saved;
    /// Create the result before taking the filter lock.
    fn create_array(&mut self) -> Self::Array;
    /// Enter the source's recursive filter mutex.
    fn lock(&mut self);
    /// Read the current list length while locked.
    fn count(&self) -> usize;
    /// Serialize the current filter at this index while locked.
    fn save(&mut self, index: usize) -> Self::Saved;
    /// Append and retain the saved object in the array.
    fn push(&mut self, array: &mut Self::Array, saved: &Self::Saved);
    /// Release the serializer's temporary reference.
    fn release(&mut self, saved: Self::Saved);
    /// Leave the source's filter mutex.
    fn unlock(&mut self);
}

/// Reject an absent source; otherwise save every filter in forward list order.
pub fn backup<B: Backend>(
    _source: Option<&mut B>,
    _log: impl FnOnce(&'static str),
) -> Option<B::Array> {
    todo!("obs_source_backup_filters")
}
