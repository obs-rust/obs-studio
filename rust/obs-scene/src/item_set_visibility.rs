//! Immediate scene-item visibility reconciliation, including queued-action cancellation.

/// Operations on a live item. Every operation between `lock` and `unlock`
/// runs under the item's actions mutex, including active-child changes.
pub trait VisibilityItem {
    fn lock(&mut self);
    fn unlock(&mut self);
    fn clear_actions(&mut self);
    fn has_active_refs(&mut self) -> bool;
    fn add_active_child(&mut self);
    fn remove_active_child(&mut self);
    fn set_active_refs(&mut self, visible: bool);
    fn set_visible(&mut self, visible: bool);
    fn set_user_visible(&mut self, visible: bool);
}

/// Cancel pending audio actions and reconcile immediately with the requested state.
pub fn set_visibility(item: &mut impl VisibilityItem, visible: bool) {
    let _ = (item, visible);
    todo!()
}
