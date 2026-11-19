//! Transform deferral over a C-owned atomic counter. The transform adapter
//! retains ordinary, group-child and parentless dispatch behavior.

/// Native operations on a live item, without borrowing its private layout.
pub trait TransformDeferral {
    fn increment(&self);
    /// Return this atomic decrement's result, never a later load.
    fn decrement(&self) -> isize;
    fn update_transform(&self);
}

pub fn begin(item: &impl TransformDeferral) {
    item.increment();
}

pub fn end(item: &impl TransformDeferral) {
    if item.decrement() == 0 {
        item.update_transform();
    }
}
