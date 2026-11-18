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
pub fn set<I: Item>(_item: Option<&mut I>, _alignment: u32) {
    todo!("alignment setter port")
}
