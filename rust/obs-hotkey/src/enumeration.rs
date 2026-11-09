//! Hotkey enumeration order, stale-ID filtering and lock scope.

/// Visit a linked collection, caching the successor before each callback.
pub fn hotkeys<T: Copy>(first: Option<T>, next: impl FnMut(T) -> Option<T>, visit: impl FnMut(T) -> bool) {
    let _ = (first, next, visit);
    todo!()
}

/// Resolve each saved context ID at the time it is visited; skip stale IDs.
pub fn context<T>(count: usize, id_at: impl FnMut(usize) -> usize, find: impl FnMut(usize) -> Option<T>, visit: impl FnMut(T) -> bool) {
    let _ = (count, id_at, find, visit);
    todo!()
}

/// Enumerate only after a successful lock, releasing it after callbacks finish.
pub fn locked(lock: impl FnOnce() -> bool, enumerate: impl FnOnce(), unlock: impl FnOnce()) {
    let _ = (lock, enumerate, unlock);
    todo!()
}
