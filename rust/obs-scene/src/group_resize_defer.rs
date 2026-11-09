//! Group resizing is scheduled, never performed, by this atomic protocol.
//! The scene walk owns its mutex and consumes the flag later. Begin/end must
//! not acquire that mutex or synchronously update transforms or emit signals.

/// Native atomic access to the two private fields, without borrowing the item.
pub trait ResizeDeferral {
    fn increment(&self);
    /// Return the value of this decrement, not a later independent load.
    fn decrement(&self) -> isize;
    fn schedule_resize(&self);
}

pub fn begin(item: &impl ResizeDeferral) {
    todo!()
}

pub fn end(item: &impl ResizeDeferral) {
    todo!()
}
