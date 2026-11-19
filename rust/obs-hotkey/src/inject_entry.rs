//! One injected event, captured under the registry lock and shared by its walk.
use crate::press::KeyCombination;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Event {
    pub hotkey: KeyCombination,
    pub pressed: bool,
    pub strict_modifiers: bool,
}

/// The existing C-owned binding walk retains its snapshot/mutation semantics.
pub trait Registry {
    fn lock(&mut self) -> bool;
    fn strict_modifiers(&self) -> bool;
    fn bindings(&mut self, event: Event);
    fn unlock(&mut self);
}

/// Dispatch even false events; the existing binding engine decides their effect.
pub fn inject(registry: &mut impl Registry, hotkey: KeyCombination, pressed: bool) {
    if !registry.lock() {
        return;
    }
    let event = Event {
        hotkey,
        pressed,
        strict_modifiers: registry.strict_modifiers(),
    };
    registry.bindings(event);
    registry.unlock();
}

/// The binding adapter always continues the current enumeration.
pub fn forward<B>(event: Event, binding: B, inject: impl FnOnce(B, Event)) -> bool {
    inject(binding, event);
    true
}
