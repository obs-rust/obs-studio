//! Tick registration policy over the C-owned array and recursive draw lock.
//!
//! New registrations occupy index zero. The graphics thread dispatches from
//! the end, so ordinary callbacks run oldest first. Removal erases just the
//! first stored pair (the newest duplicate). The same recursive lock covers
//! each operation and the entire C dispatch; callbacks may reenter registration.

/// Storage operations, performed only while the existing draw lock is held.
pub trait Registry {
    /// Callback identity includes both function and parameter.
    type Entry: Copy + Eq;
    fn lock(&mut self);
    fn unlock(&mut self);
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool { self.len() == 0 }
    fn get(&self, index: usize) -> Self::Entry;
    fn insert(&mut self, index: usize, entry: Self::Entry);
    fn erase(&mut self, index: usize);
}

/// Add without deduplicating, while holding the existing C recursive mutex.
pub fn add<R: Registry>(_registry: &mut R, _entry: R::Entry) {
    todo!()
}

/// Remove the first stored matching pair; missing entries are harmless.
pub fn remove<R: Registry>(_registry: &mut R, _entry: R::Entry) {
    todo!()
}
