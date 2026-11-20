//! The background identifier of one key combination, from the backend's
//! callback. The callback table and its mutex stay with the caller.

/// Access to the C-owned callback slot and the identifier string.
pub trait CombinationBackgroundId {
    /// The saved callback.
    type Callback: Copy;
    /// The identifier being built, empty when created.
    type Id;
    /// What the caller receives on success.
    type Owned;
    fn lock(&mut self) -> bool;
    /// The current callback; read only while the lock is held.
    fn callback(&mut self) -> Option<Self::Callback>;
    fn unlock(&mut self);
    /// A new, empty identifier.
    fn empty(&mut self) -> Self::Id;
    /// Invoke the saved callback once, after the lock is released.
    fn invoke(&mut self, callback: Self::Callback, id: &mut Self::Id) -> bool;
    /// Release a partial identifier after a missing callback or a failure.
    fn free(&mut self, id: Self::Id);
    /// Hand a successful identifier to the caller unchanged.
    fn transfer(&mut self, id: Self::Id) -> Self::Owned;
}

/// Capture the callback under the lock and call it with the lock released.
/// A failed lock returns `None` with no further access.
pub fn query<B: CombinationBackgroundId>(backend: &mut B) -> Option<B::Owned> {
    if !backend.lock() {
        return None;
    }
    let mut id = backend.empty();
    let callback = backend.callback();
    backend.unlock();
    match callback {
        Some(callback) if backend.invoke(callback, &mut id) => Some(backend.transfer(id)),
        _ => {
            backend.free(id);
            None
        }
    }
}
