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
pub fn hotkeys<R: Registry>(registry: &mut R, mut callback: impl FnMut(usize, R::Hotkey) -> bool) {
    if !registry.lock() {
        return;
    }
    let mut current = registry.first();
    while let Some(key) = current {
        // HASH_ITER saves next before the callback, which may remove key.
        current = registry.next(key);
        if !callback(registry.id(key), key) {
            break;
        }
    }
    registry.unlock();
}

/// Enumerate binding indices under the same mutex as public hotkey enumeration.
pub fn bindings<R: Registry>(
    registry: &mut R,
    mut callback: impl FnMut(usize, R::Binding) -> bool,
) {
    if !registry.lock() {
        return;
    }
    registry.bindings(&mut callback);
    registry.unlock();
}

/// Enumerate a context under its caller's lock, skipping IDs no longer live.
pub fn context<C: Context>(context: &C, mut callback: impl FnMut(C::Hotkey) -> bool) {
    let (count, snapshot) = context.snapshot();
    for index in 0..count {
        if let Some(key) = context.find(context.id(&snapshot, index))
            && !callback(key)
        {
            break;
        }
    }
}
