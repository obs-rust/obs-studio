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
pub fn reconcile(_item: &mut impl Item, _visible: bool) {
    todo!()
}
