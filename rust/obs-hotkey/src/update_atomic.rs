//! Synchronous updates under the existing recursive hotkey mutex.

/// The registry's fallible recursive lock.
pub trait AtomicUpdate {
    fn lock(&mut self) -> bool;
    fn unlock(&mut self);
}

/// Run the update exactly once if the registry can be locked.
pub fn update<B: AtomicUpdate>(_registry: &mut B, _callback: impl FnOnce(&mut B)) {
    todo!()
}
