//! Acquire a source's weak handle: borrow its control and add one weak reference.

/// C-owned control read and weak-reference count (`weak_refs`).
pub trait Host {
    type Source: Copy;
    type Weak: Copy;
    /// Borrow a live source's control token, which may be absent.
    fn control(&self, source: Self::Source) -> Option<Self::Weak>;
    /// Add one weak reference; an absent control is a no-op.
    fn addref(&self, weak: Option<Self::Weak>);
}

/// `None` is a NULL source, which returns NULL without touching the host.
/// Otherwise the control is returned after one addref, as in the C, which
/// passes even an absent control to the no-op addref.
pub fn get_weak_source<H: Host>(host: &H, source: Option<H::Source>) -> Option<H::Weak> {
    let weak = host.control(source?);
    host.addref(weak);
    weak
}
