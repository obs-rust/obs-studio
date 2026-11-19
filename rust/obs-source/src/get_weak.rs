//! Acquire a source's weak handle: borrow its control and add one weak reference.

/// C-owned control read and weak-reference count; release stays in C.
pub trait Host {
    type Source: Copy;
    type Weak: Copy;
    /// Borrow a live source's control token, which may be absent.
    fn control(&self, source: Self::Source) -> Option<Self::Weak>;
    /// Add one weak reference; an absent control is a no-op.
    fn addref(&self, weak: Option<Self::Weak>);
}

/// `None` is a NULL source, which returns NULL without touching the host.
pub fn get_weak_source<H: Host>(host: &H, source: Option<H::Source>) -> Option<H::Weak> {
    let _ = (host, source);
    todo!("RED: source weak-handle acquisition")
}
