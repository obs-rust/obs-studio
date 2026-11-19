//! Transform deferral over a C-owned atomic counter. The transform adapter
//! retains ordinary, group-child and parentless dispatch behavior.

/// Native operations on a live item, without borrowing its private layout.
pub trait TransformDeferral {
    fn increment(&self);
    /// Return this atomic decrement's result, never a later load.
    fn decrement(&self) -> isize;
    fn update_transform(&self);
}

pub fn begin(_item: &impl TransformDeferral) {
    todo!("port obs_sceneitem_defer_update_begin");
}

pub fn end(_item: &impl TransformDeferral) {
    todo!("port obs_sceneitem_defer_update_end");
}
