//! Alignment storage with the scene's captured-parent locking and update boundary.

/// Operations supplied by a live scene item or a safe test model.
pub trait Item {
    /// The parent captured while locking the fields.
    type Parent;
    /// Capture the parent and lock its fields.
    fn lock(&mut self) -> Self::Parent;
    /// Store every alignment bit under the lock.
    fn store(&mut self, alignment: u32);
    /// Release the captured parent, even if the item's parent changed.
    fn unlock(&mut self, parent: Self::Parent);
    /// Request a transform update after unlocking, including unchanged values.
    fn update(&mut self);
}

/// A missing item is a silent no-op.
pub fn set<I: Item>(item: Option<&mut I>, alignment: u32) {
    let Some(item) = item else { return };
    let parent = item.lock();
    item.store(alignment);
    item.unlock(parent);
    item.update();
}
