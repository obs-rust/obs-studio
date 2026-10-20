//! Safe core of `libobs/obs-nix-hotkey-state.c.h`: the key state the libobs
//! hotkey thread reads on Wayland.
//!
//! Wayland gives a client no way to query the keyboard, so the state is fed
//! from what does exist: keys held while an OBS window has keyboard focus,
//! and the global shortcuts the xdg-desktop-portal reports as activated. A
//! shortcut the portal releases before a hotkey poll cycle has seen it stays
//! until one has (`release_pending`), so a short tap still presses the
//! hotkey.
//!
//! The functions are generic over where the state lives ([`Store`]), so the
//! C ABI shim runs them on the C struct and its `bmalloc` darray, and over
//! the lock ([`HotkeyLock`]), taken and released exactly where the C takes
//! `obs->hotkeys.mutex`.

use crate::name_map::{KEY_NONE, OBS_KEY_ALT, OBS_KEY_CONTROL, OBS_KEY_META, OBS_KEY_SHIFT};

pub use crate::name_map::OBS_KEY_LAST_VALUE;

/// `INTERACT_SHIFT_KEY` (`obs-interaction.h`).
pub const INTERACT_SHIFT_KEY: u32 = 1 << 1;
/// `INTERACT_CONTROL_KEY`.
pub const INTERACT_CONTROL_KEY: u32 = 1 << 2;
/// `INTERACT_ALT_KEY`.
pub const INTERACT_ALT_KEY: u32 = 1 << 3;
/// `INTERACT_COMMAND_KEY`.
pub const INTERACT_COMMAND_KEY: u32 = 1 << 7;

/// `obs_key_combination_t`: `INTERACT_*_KEY` bits and an `obs_key_t`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct KeyCombination {
    pub modifiers: u32,
    pub key: i32,
}

/// `struct obs_nix_hotkey_shortcut`: a shortcut the portal reported as
/// activated.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Shortcut {
    pub combo: KeyCombination,
    /// A hotkey poll cycle has run while this entry was present.
    pub observed: bool,
    /// The portal already reported the release; the entry waits for a poll
    /// cycle to see it.
    pub release_pending: bool,
}

/// The shortcut list, in insertion order (`DARRAY(struct
/// obs_nix_hotkey_shortcut)` in C).
pub trait Shortcuts {
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Entry `idx`, which is below `len()`.
    fn get(&self, idx: usize) -> Shortcut;
    /// Replaces entry `idx`, which is below `len()`.
    fn set(&mut self, idx: usize, shortcut: Shortcut);
    /// `da_push_back`.
    fn push(&mut self, shortcut: Shortcut);
    /// `da_erase`: removes entry `idx`, keeping the order of the rest.
    fn erase(&mut self, idx: usize);
    /// `da_free`: removes every entry and releases the storage.
    fn free(&mut self);
}

impl Shortcuts for Vec<Shortcut> {
    fn len(&self) -> usize {
        Vec::len(self)
    }
    fn get(&self, idx: usize) -> Shortcut {
        self[idx]
    }
    fn set(&mut self, idx: usize, shortcut: Shortcut) {
        self[idx] = shortcut;
    }
    fn push(&mut self, shortcut: Shortcut) {
        Vec::push(self, shortcut);
    }
    fn erase(&mut self, idx: usize) {
        self.remove(idx);
    }
    fn free(&mut self) {
        *self = Vec::new();
    }
}

/// Where the state lives: `struct obs_nix_hotkey_state`.
pub trait Store {
    type Shortcuts: Shortcuts;
    /// `OBS_KEY_LAST_VALUE` entries, indexed by `obs_key_t`.
    fn focused_keys(&mut self) -> &mut [bool];
    fn focused_modifiers(&mut self) -> &mut u32;
    fn shortcuts(&mut self) -> &mut Self::Shortcuts;
}

/// `obs->hotkeys.mutex`, taken through `obs_hotkeys_mutex_lock` and
/// `obs_hotkeys_mutex_unlock`. It is recursive: the hotkey thread holds it
/// for a whole poll cycle and calls [`is_pressed`] inside.
pub trait HotkeyLock {
    fn lock(&self);
    fn unlock(&self);
}

/// A lock that does nothing, for state no other thread sees.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoLock;

impl HotkeyLock for NoLock {
    fn lock(&self) {}
    fn unlock(&self) {}
}

/// An owned state, for Rust callers and tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyState {
    pub focused_keys: Vec<bool>,
    pub focused_modifiers: u32,
    pub shortcuts: Vec<Shortcut>,
}

impl Default for KeyState {
    /// All keys up, no modifiers, no shortcuts (C: a zeroed struct).
    fn default() -> Self {
        Self {
            focused_keys: vec![false; OBS_KEY_LAST_VALUE],
            focused_modifiers: 0,
            shortcuts: Vec::new(),
        }
    }
}

impl Store for KeyState {
    type Shortcuts = Vec<Shortcut>;
    fn focused_keys(&mut self) -> &mut [bool] {
        &mut self.focused_keys
    }
    fn focused_modifiers(&mut self) -> &mut u32 {
        &mut self.focused_modifiers
    }
    fn shortcuts(&mut self) -> &mut Vec<Shortcut> {
        &mut self.shortcuts
    }
}

/// `obs_nix_hotkey_state_free`: drops every shortcut. Takes no lock, as in C.
pub fn free(state: &mut impl Store) {
    let _ = state;
    todo!()
}

/// `obs_nix_hotkey_state_set_key`: a `wl_keyboard` key event. Codes outside
/// `OBS_KEY_NONE < key < OBS_KEY_LAST_VALUE` are ignored without locking.
pub fn set_key(state: &mut impl Store, lock: &impl HotkeyLock, key: i32, pressed: bool) {
    let _ = (state, lock, key, pressed);
    todo!()
}

/// `obs_nix_hotkey_state_set_modifiers`: the keyboard's `INTERACT_*_KEY`
/// bits.
pub fn set_modifiers(state: &mut impl Store, lock: &impl HotkeyLock, modifiers: u32) {
    let _ = (state, lock, modifiers);
    todo!()
}

/// `obs_nix_hotkey_state_clear_keys`: keyboard focus lost; every key and
/// modifier is released. Shortcuts stay.
pub fn clear_keys(state: &mut impl Store, lock: &impl HotkeyLock) {
    let _ = (state, lock);
    todo!()
}

/// `obs_nix_hotkey_state_set_shortcut`: the portal's `Activated` (`active`)
/// or `Deactivated` for one key combination.
pub fn set_shortcut(
    state: &mut impl Store,
    lock: &impl HotkeyLock,
    combo: KeyCombination,
    active: bool,
) {
    let _ = (state, lock, combo, active);
    todo!()
}

/// `obs_nix_hotkey_state_flush`: drops released shortcuts that a poll cycle
/// has seen. True while a released one still waits to be seen.
pub fn flush(state: &mut impl Store, lock: &impl HotkeyLock) -> bool {
    let _ = (state, lock);
    todo!()
}

/// `obs_nix_hotkey_state_is_pressed`: the answer for
/// `obs_hotkeys_platform_is_pressed`. Marks every shortcut as observed.
pub fn is_pressed(state: &mut impl Store, lock: &impl HotkeyLock, key: i32) -> bool {
    let _ = (state, lock, key, KEY_NONE);
    todo!()
}

/// The `INTERACT_*_KEY` bit a modifier key stands for, 0 for other keys.
#[must_use]
pub fn modifier_for_key(key: i32) -> u32 {
    let _ = (
        OBS_KEY_SHIFT,
        OBS_KEY_CONTROL,
        OBS_KEY_ALT,
        OBS_KEY_META,
        key,
    );
    todo!()
}
