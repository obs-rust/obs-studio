//! Tick registration policy over the C-owned array and recursive draw lock.
//!
//! New registrations occupy index zero. The graphics thread dispatches from
//! the end, so ordinary callbacks run oldest first. Removal erases just the
//! first stored pair (the newest duplicate). The same recursive lock covers
//! each operation and the entire C dispatch; callbacks may reenter registration.
//! Dispatch deliberately keeps C's live reverse traversal, not a snapshot:
//! insertion from a callback can skip an existing row and visit the new row in
//! that tick. This port changes registration only, not those dispatch semantics.

/// Storage operations, performed only while the existing draw lock is held.
pub trait Registry {
    /// Callback identity includes both function and parameter.
    type Entry: Copy + Eq;
    fn lock(&mut self);
    fn unlock(&mut self);
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    fn get(&self, index: usize) -> Self::Entry;
    fn insert(&mut self, index: usize, entry: Self::Entry);
    fn erase(&mut self, index: usize);
}

/// Add without deduplicating, while holding the existing C recursive mutex.
pub fn add<R: Registry>(registry: &mut R, entry: R::Entry) {
    registry.lock();
    registry.insert(0, entry);
    registry.unlock();
}

/// Remove the first stored matching pair; missing entries are harmless.
pub fn remove<R: Registry>(registry: &mut R, entry: R::Entry) {
    registry.lock();
    for index in 0..registry.len() {
        if registry.get(index) == entry {
            registry.erase(index);
            break;
        }
    }
    registry.unlock();
}
