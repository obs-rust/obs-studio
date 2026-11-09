//! Hotkey enumeration order, stale-ID filtering and lock scope.

/// Visit a linked collection, caching the successor before each callback.
pub fn hotkeys<T: Copy>(
    mut first: Option<T>,
    mut next: impl FnMut(T) -> Option<T>,
    mut visit: impl FnMut(T) -> bool,
) {
    while let Some(key) = first {
        first = next(key);
        if !visit(key) {
            break;
        }
    }
}

/// Resolve each saved context ID at the time it is visited; skip stale IDs.
pub fn context<T>(
    count: usize,
    mut id_at: impl FnMut(usize) -> usize,
    mut find: impl FnMut(usize) -> Option<T>,
    mut visit: impl FnMut(T) -> bool,
) {
    for index in 0..count {
        let Some(key) = find(id_at(index)) else {
            continue;
        };
        if !visit(key) {
            break;
        }
    }
}

/// Enumerate only after a successful lock, releasing it after callbacks finish.
pub fn locked(lock: impl FnOnce() -> bool, enumerate: impl FnOnce(), unlock: impl FnOnce()) {
    if !lock() {
        return;
    }
    enumerate();
    unlock();
}
