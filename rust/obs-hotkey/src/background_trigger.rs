//! Owned background shortcut triggers, dispatched under the hotkey mutex.
use crate::nix_hotkey_state::KeyCombination;

/// The private core, its mutex, and one callback returning owned storage.
pub trait BackgroundTrigger {
    type Trigger;
    fn lock(&mut self) -> bool;
    fn trigger(&mut self, combo: KeyCombination) -> (bool, Self::Trigger);
    fn unlock(&mut self);
}

/// Preserve callback ownership, including success without allocated storage.
pub fn query<B: BackgroundTrigger>(
    background: &mut B,
    combo: KeyCombination,
) -> Option<B::Trigger> {
    if !background.lock() {
        return None;
    }
    let (found, trigger) = background.trigger(combo);
    background.unlock();
    if found { Some(trigger) } else { None }
}
