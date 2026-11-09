//! Synchronous updates under the existing recursive hotkey mutex.

/// The registry's fallible recursive lock.
pub trait AtomicUpdate {
    fn lock(&mut self) -> bool;
    fn unlock(&mut self);
}

/// Run the update exactly once if the registry can be locked.
pub fn update<B: AtomicUpdate>(registry: &mut B, callback: impl FnOnce(&mut B)) {
    if !registry.lock() {
        return;
    }
    callback(registry);
    registry.unlock();
}
