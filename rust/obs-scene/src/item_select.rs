//! Scene item selection, independent of the C layout.

/// Live-item operations; store completes before synchronous notification.
pub trait Item {
    fn selected(&self) -> bool;
    fn has_parent(&self) -> bool;
    fn store(&self, selected: bool);
    fn signal(&self, selected: bool);
}

/// Missing, unchanged and detached items do nothing. Callbacks may reenter.
pub fn select(item: Option<&impl Item>, requested: bool) {
    let Some(item) = item else {
        return;
    };
    if item.selected() == requested || !item.has_parent() {
        return;
    }
    item.store(requested);
    item.signal(requested);
}
