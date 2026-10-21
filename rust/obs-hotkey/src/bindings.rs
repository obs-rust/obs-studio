//! Safe core of `libobs/obs-hotkey-bindings.c.h`: the hotkey binding
//! codec.
//!
//! One item of a hotkey's saved bindings array, in every profile and scene
//! collection, is an object of four modifier bools (`"shift"`,
//! `"control"`, `"alt"` and `"command"`) and a key name (`"key"`). The
//! codec turns such an item into an `obs_key_combination_t` and back:
//!
//! - [`combo_from_fields`] is the parse of `load_binding()`: the
//!   `INTERACT_*_KEY` bit of each modifier that reads as true (`load_modifier`)
//!   and the key from its name;
//! - [`combo_bindable`] is the rule of `create_binding()`: the primary mouse
//!   buttons are reserved for UI interaction;
//! - [`combo_to_fields`] is the per-binding body of
//!   `save_bindings_helper()`: one `true` per modifier bit that is set
//!   (`save_modifier`), then the key name, always in the order of
//!   [`MODIFIERS`] and [`KEY_FIELD`] last.
//!
//! The obs_data reads and writes stay at the C boundary (`ffi::bindings`),
//! so this module never sees an obs_data: a modifier is true when
//! `obs_data_get_bool` says so (only a JSON `true` does), and the key name
//! is what `obs_data_get_string` returns (`""` for a missing or non-string
//! key). The input comes from scene collections and profiles, which can
//! come from others, so nothing here can panic, index out of bounds or
//! overflow.
//!
//! Byte for byte the C: [`combo_from_fields`] drops an item with no
//! modifiers and no known key (`{}`, an unknown, empty or missing key, or
//! `OBS_KEY_NONE`), which could never fire, and keeps a modifier-only
//! binding (key `OBS_KEY_NONE` with modifiers), which can.

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

use std::ffi::CStr;

use crate::name_map::{self, KEY_NONE, OBS_KEY_MOUSE1, OBS_KEY_MOUSE2, key_from_name};
use crate::nix_hotkey_state::{
    INTERACT_ALT_KEY, INTERACT_COMMAND_KEY, INTERACT_CONTROL_KEY, INTERACT_SHIFT_KEY,
    KeyCombination,
};

/// One modifier of a saved binding: its obs_data key and its
/// `INTERACT_*_KEY` bit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Modifier {
    /// The obs_data key (`"shift"`, ...).
    pub name: &'static CStr,
    /// The `INTERACT_*_KEY` bit.
    pub flag: u32,
}

/// The four modifiers, in the order the C reads and writes them.
pub const MODIFIERS: [Modifier; 4] = [
    Modifier {
        name: c"shift",
        flag: INTERACT_SHIFT_KEY,
    },
    Modifier {
        name: c"control",
        flag: INTERACT_CONTROL_KEY,
    },
    Modifier {
        name: c"alt",
        flag: INTERACT_ALT_KEY,
    },
    Modifier {
        name: c"command",
        flag: INTERACT_COMMAND_KEY,
    },
];

/// The obs_data key of the key name, written after the modifiers.
pub const KEY_FIELD: &CStr = c"key";

/// One field [`combo_to_fields`] writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    /// `obs_data_set_bool(name, true)` for a set modifier bit.
    Modifier(&'static CStr),
    /// `obs_data_set_string("key", obs_key_to_name(key))`, last.
    Key(i32),
}

/// The fields of one saved binding, in the order they are written: the
/// set modifiers in [`MODIFIERS`] order, then the key. Fused.
#[derive(Debug, Clone)]
pub struct Fields {
    combo: KeyCombination,
    /// The next entry of [`MODIFIERS`] to look at; `MODIFIERS.len()` for
    /// the key, past it when done.
    next: usize,
}

impl Iterator for Fields {
    type Item = Field;

    fn next(&mut self) -> Option<Field> {
        while let Some(m) = MODIFIERS.get(self.next) {
            self.next = self.next.saturating_add(1);
            // save_modifier
            if self.combo.modifiers & m.flag == m.flag {
                return Some(Field::Modifier(m.name));
            }
        }
        if self.next == MODIFIERS.len() {
            self.next = self.next.saturating_add(1);
            return Some(Field::Key(self.combo.key));
        }
        None
    }
}

impl std::iter::FusedIterator for Fields {}

/// The parse of `load_binding()` (`obs_hotkey_combination_from_data`):
/// `modifiers` holds what `obs_data_get_bool` read for each of
/// [`MODIFIERS`], in that order, and `key_name` what `obs_data_get_string`
/// read for [`KEY_FIELD`].
///
/// `None` for an item with no modifiers and no known key: it is no
/// binding, as it could never fire. A modifier-only binding has key
/// `OBS_KEY_NONE`.
pub fn combo_from_fields(modifiers: [bool; 4], key_name: &CStr) -> Option<KeyCombination> {
    let mut combo = KeyCombination::default();
    // load_modifier
    for (m, set) in MODIFIERS.iter().zip(modifiers) {
        if set {
            combo.modifiers |= m.flag;
        }
    }
    combo.key = key_from_name(key_name);
    // OBS_KEY_NONE, or not a key (>= OBS_KEY_LAST_VALUE), with no modifiers.
    if combo.modifiers == 0 && (combo.key == KEY_NONE || name_map::key_name(combo.key).is_none()) {
        return None;
    }
    Some(combo)
}

/// The rule of `create_binding()` (`obs_hotkey_combination_bindable`), for
/// JSON and API bindings alike.
///
/// Match the frontend: the primary mouse buttons are reserved for UI
/// interaction. Any other key code, in range or not, and any modifiers are
/// bindable.
pub fn combo_bindable(combo: KeyCombination) -> bool {
    combo.key != OBS_KEY_MOUSE1 && combo.key != OBS_KEY_MOUSE2
}

/// The per-binding body of `save_bindings_helper()`
/// (`obs_hotkey_combination_to_data`): the fields to write for `combo`.
pub fn combo_to_fields(combo: KeyCombination) -> Fields {
    Fields { combo, next: 0 }
}
