//! Live registry enumeration, preserving the C callback and lock boundaries.

/// Private registry access. Handles are copied, never borrowed over callbacks.
pub trait Registry {
    type Hotkey: Copy;
    type Binding;
    fn lock(&mut self) -> bool;
    fn unlock(&mut self);
    fn first(&self) -> Option<Self::Hotkey>;
    fn next(&self, key: Self::Hotkey) -> Option<Self::Hotkey>;
    fn id(&self, key: Self::Hotkey) -> usize;
    /// Reuse the existing binding walk, including its array snapshot.
    fn bindings(&mut self, callback: &mut dyn FnMut(usize, Self::Binding) -> bool);
}

/// Snapshot only the context array header; resolve each stored ID live.
pub trait Context {
    type Hotkey;
    type Snapshot;
    fn snapshot(&self) -> (usize, Self::Snapshot);
    fn id(&self, snapshot: &Self::Snapshot, index: usize) -> usize;
    fn find(&self, id: usize) -> Option<Self::Hotkey>;
}

/// Enumerate hash insertion order while holding the recursive hotkey mutex.
pub fn hotkeys<R: Registry>(_registry: &mut R, _callback: impl FnMut(usize, R::Hotkey) -> bool) {
    todo!()
}

/// Enumerate binding indices under the same mutex as public hotkey enumeration.
pub fn bindings<R: Registry>(_registry: &mut R, _callback: impl FnMut(usize, R::Binding) -> bool) {
    todo!()
}

/// Enumerate a context under its caller's lock, skipping IDs no longer live.
pub fn context<C: Context>(_context: &C, _callback: impl FnMut(C::Hotkey) -> bool) {
    todo!()
}
