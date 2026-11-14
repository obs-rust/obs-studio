//! Safe core of `libobs/obs-key-combination-host-posix.c.h`: how Linux and
//! FreeBSD write a key combination as text (`obs_key_combination_to_str()`).
//!
//! Each pressed modifier, then the key, is named through `obs_key_to_str()`
//! (still C, the X11 or Wayland platform's) and appended. The order is
//! Control, Meta (`INTERACT_COMMAND_KEY`), Alt, Shift, then the key unless it
//! is `OBS_KEY_NONE`. A name that comes back empty is left out; `" + "` goes
//! before a name only when the text already has something in it, so a
//! caller's prefilled text is appended to. Every other modifier bit is
//! ignored.
//!
//! The text is a `struct dstr`, so the function is generic over
//! [`Text`]: the C ABI shim runs it on the caller's `dstr`, with a fresh
//! `dstr` per name that is freed after it, as the C does.

use crate::name_map::{KEY_NONE, OBS_KEY_ALT, OBS_KEY_CONTROL, OBS_KEY_META, OBS_KEY_SHIFT};
use crate::nix_hotkey_state::{
    INTERACT_ALT_KEY, INTERACT_COMMAND_KEY, INTERACT_CONTROL_KEY, INTERACT_SHIFT_KEY,
    KeyCombination,
};
pub use crate::portal_shortcut_ids::Text;

/// The modifiers that are named, in the order they are written: the
/// `INTERACT_*_KEY` bit and the key that names it.
pub const MODIFIER_KEYS: [(u32, i32); 4] = [
    (INTERACT_CONTROL_KEY, OBS_KEY_CONTROL),
    (INTERACT_COMMAND_KEY, OBS_KEY_META),
    (INTERACT_ALT_KEY, OBS_KEY_ALT),
    (INTERACT_SHIFT_KEY, OBS_KEY_SHIFT),
];

/// `obs_key_combination_to_str`: appends the names of `combination`'s
/// modifiers and key to `text`. `key_to_str` is `obs_key_to_str()`: it
/// writes one key's name into an empty text.
pub fn combination_to_str<T, F>(combination: KeyCombination, mut key_to_str: F, text: &mut T)
where
    T: Text + Default,
    F: FnMut(i32, &mut T),
{
    let _ = (combination, &mut key_to_str, text);
    todo!("obs_key_combination_to_str")
}
