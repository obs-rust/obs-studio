//! Safe core of `libobs/obs-hotkey-press.c.h`: the hotkey press state
//! machine, which decides when one binding is pressed and when it is
//! released.
//!
//! It runs for every hotkey on every OS, push-to-talk and push-to-mute
//! included, on two paths:
//! - the hotkey thread polls the platform key state every 25 ms: it gathers
//!   the held modifiers once ([`query_modifiers`]) and then steps every
//!   binding ([`query`]);
//! - `obs_hotkey_inject_event()` (the frontend while OBS has focus, and
//!   obs-websocket's `TriggerHotkeyByKeySequence`) steps every binding with
//!   one key event ([`inject`]). It only ever presses; the next poll
//!   releases.
//!
//! A step reads the binding's key combination and pressed flag, may change
//! its `modifiers_match` latch, and returns the [`Action`] the caller takes:
//! libobs's `obs_hotkey_binding_press()` or `obs_hotkey_binding_release()`,
//! which set the pressed flag, count the hotkey's pressed bindings and call
//! its callback. The core never sets the pressed flag itself, keeps no state
//! between calls (it all lives in the binding) and takes no lock: the C
//! caller holds the hotkey mutex.
//!
//! The key state is a callback, called at most once per step and only at
//! the point the C calls `obs_hotkey_key_is_pressed()`: after the
//! modifier and latch checks pass, before `no_press` is looked at, and never
//! for a modifier-only or an empty binding. On X11 each call is a round trip
//! to the X server, so it is never precomputed.
//!
//! Characterized, not endorsed (`test_hotkey_press_state`,
//! `test_hotkey_press_steps`), and kept as the C has them:
//! - a key pressed before its modifiers still fires: the latch delays the
//!   press by one poll but does not enforce the order;
//! - an injected event of any key with exactly a modifier-only binding's
//!   modifiers presses it, a release as well as a press;
//! - an empty binding (no key, no modifiers) is pressed by every injected
//!   event without modifiers (every one at all, not strict), and released
//!   by the next poll;
//! - with rerouting on and no router, a press still counts (this one is in
//!   the C press wrapper, not here).
//!
//! Keys come from the OS, from obs-websocket and from plugins, so a key is
//! any `i32`: the core only compares keys and never indexes by one.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable
)]

use crate::name_map::{KEY_NONE, OBS_KEY_ALT, OBS_KEY_CONTROL, OBS_KEY_META, OBS_KEY_SHIFT};
pub use crate::nix_hotkey_state::KeyCombination;
use crate::nix_hotkey_state::{
    INTERACT_ALT_KEY, INTERACT_COMMAND_KEY, INTERACT_CONTROL_KEY, INTERACT_SHIFT_KEY,
};

/// The modifier keys `obs_hotkeys_query_modifiers()` queries, in its order,
/// with the `INTERACT_*_KEY` bit each one sets. Meta is the command bit.
pub const MODIFIER_KEYS: [(i32, u32); 4] = [
    (OBS_KEY_SHIFT, INTERACT_SHIFT_KEY),
    (OBS_KEY_CONTROL, INTERACT_CONTROL_KEY),
    (OBS_KEY_ALT, INTERACT_ALT_KEY),
    (OBS_KEY_META, INTERACT_COMMAND_KEY),
];

/// The fields of one binding the state machine reads and writes
/// (`struct obs_hotkey_binding_fields` in C points at them).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Binding {
    /// The bound key combination. A key of `OBS_KEY_NONE` with modifiers is
    /// a modifier-only binding; with none it is an empty binding.
    pub key: KeyCombination,
    /// Whether the binding is pressed. Read only: the caller's press and
    /// release set it.
    pub pressed: bool,
    /// The latch: a key binding is pressed only on a poll where its
    /// modifiers match and also matched on the step before.
    pub modifiers_match: bool,
}

/// What the caller does with the binding after a step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Nothing.
    None,
    /// `obs_hotkey_binding_press()`.
    Press,
    /// `obs_hotkey_binding_release()`.
    Release,
}

/// `modifiers_match()`: strict, the binding's modifiers equal the held
/// ones; not strict, they are a subset of them.
pub fn modifiers_match(binding_modifiers: u32, modifiers: u32, strict: bool) -> bool {
    if strict {
        binding_modifiers == modifiers
    } else {
        binding_modifiers & modifiers == binding_modifiers
    }
}

/// `obs_hotkey_binding_query()` (`handle_binding()`): one poll of one
/// binding, with the held `modifiers` from [`query_modifiers`]. `no_press`
/// (background presses off) blocks presses but not releases. `is_pressed`
/// is the platform key state, called at most once, at the point the C
/// calls it.
///
/// The C's `bool *pressed` parameter is gone: every caller passed `NULL`,
/// and it was only ever aliased to the modifier-only flag, so the key state
/// is skipped for a modifier-only binding here, as there.
pub fn query(
    binding: &mut Binding,
    modifiers: u32,
    no_press: bool,
    strict: bool,
    is_pressed: impl FnOnce(i32) -> bool,
) -> Action {
    let key = binding.key;
    let matches = modifiers_match(key.modifiers, modifiers, strict);
    let modifiers_only = key.key == KEY_NONE;

    if !strict && key.modifiers == 0 {
        binding.modifiers_match = true;
    }

    let held = if key.modifiers == 0 && modifiers_only {
        // An empty binding: never pressed by a poll.
        false
    } else if (!binding.modifiers_match && !modifiers_only) || !matches {
        false
    } else {
        // Lazily, and only for a binding with a key.
        modifiers_only || is_pressed(key.key)
    };

    if held {
        if binding.pressed || no_press {
            Action::None
        } else {
            Action::Press
        }
    } else {
        binding.modifiers_match = matches;
        if binding.pressed {
            Action::Release
        } else {
            Action::None
        }
    }
}

/// `obs_hotkey_binding_inject()`: one injected key event for one binding.
/// It presses when the modifiers match, and the key is the binding's (or
/// the binding has no key), and the event is a press (or the binding has
/// no key); then it also sets the latch, pressed or not. It never releases
/// and never needs the key state.
pub fn inject(binding: &mut Binding, event: KeyCombination, pressed: bool, strict: bool) -> Action {
    if !modifiers_match(binding.key.modifiers, event.modifiers, strict) {
        return Action::None;
    }
    let presses = binding.key.key == KEY_NONE || (binding.key.key == event.key && pressed);
    if !presses {
        return Action::None;
    }
    binding.modifiers_match = true;
    if binding.pressed {
        Action::None
    } else {
        Action::Press
    }
}

/// `obs_hotkeys_query_modifiers()`: the held modifiers, querying Shift,
/// Control, Alt and Meta once each, in that order ([`MODIFIER_KEYS`]).
pub fn query_modifiers(mut is_pressed: impl FnMut(i32) -> bool) -> u32 {
    let mut modifiers = 0;
    for (key, bit) in MODIFIER_KEYS {
        if is_pressed(key) {
            modifiers |= bit;
        }
    }
    modifiers
}
