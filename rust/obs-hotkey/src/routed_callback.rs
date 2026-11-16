//! Synchronous routed dispatch through the existing hotkey registry.

/// Registry operations, all field access and callbacks occurring under its lock.
pub trait Registry {
    type Hotkey;
    fn lock(&mut self) -> bool;
    fn enabled(&self) -> bool;
    fn find(&mut self, id: usize) -> Option<Self::Hotkey>;
    fn call(&mut self, hotkey: Self::Hotkey, id: usize, pressed: bool);
    fn unlock(&mut self);
}

/// Dispatch directly to a registered callback when rerouting is enabled.
pub fn trigger<R: Registry>(registry: &mut R, id: usize, pressed: bool) {
    if !registry.lock() {
        return;
    }
    if registry.enabled()
        && let Some(hotkey) = registry.find(id)
    {
        registry.call(hotkey, id, pressed);
    }
    registry.unlock();
}
