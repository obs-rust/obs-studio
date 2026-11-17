//! Immediate scene-item visibility reconciliation, including queued-action cancellation.

/// Operations on a live item. Every operation between `lock` and `unlock`
/// runs under the item's actions mutex, including active-child changes.
pub trait VisibilityItem {
    /// Acquire the item's actions mutex.
    fn lock(&mut self);
    /// Release the mutex after every state write.
    fn unlock(&mut self);
    /// Resize the pending audio-action queue to zero without freeing its storage.
    fn clear_actions(&mut self);
    /// Atomically read the prior signed C `long`, widened without truncation.
    fn active_refs(&mut self) -> i128;
    /// Add the source to the parent's active children, ignoring insertion failure.
    fn add_active_child(&mut self);
    /// Remove the source from the parent's active children exactly once.
    fn remove_active_child(&mut self);
    /// Atomically normalize references to one for visible, zero for hidden.
    fn set_active_refs(&mut self, visible: bool);
    /// Atomically store the effective visibility.
    fn set_visible(&mut self, visible: bool);
    /// Atomically store the requested visibility after the effective visibility.
    fn set_user_visible(&mut self, visible: bool);
}

/// Cancel pending audio actions and reconcile immediately with the requested state.
pub fn set_visibility(item: &mut impl VisibilityItem, visible: bool) {
    item.lock();
    item.clear_actions();
    if item.active_refs() > 0 {
        if !visible {
            item.remove_active_child();
        }
    } else if visible {
        item.add_active_child();
    }
    item.set_active_refs(visible);
    item.set_visible(visible);
    item.set_user_visible(visible);
    item.unlock();
}
