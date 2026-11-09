//! Immediate visibility reconciliation, serialized by the item actions mutex.

/// Operations on a live item; callers provide exclusive actions-mutex access.
pub trait Item {
    fn clear_actions(&mut self);
    fn active_refs(&self) -> i64;
    fn add_child(&mut self);
    fn remove_child(&mut self);
    fn normalize(&mut self, visible: bool);
}

/// Cancel pending audio actions and reconcile active-child membership.
pub fn reconcile(item: &mut impl Item, visible: bool) {
    item.clear_actions();
    if item.active_refs() > 0 {
        if !visible {
            item.remove_child();
        }
    } else if visible {
        item.add_child();
    }
    item.normalize(visible);
}
