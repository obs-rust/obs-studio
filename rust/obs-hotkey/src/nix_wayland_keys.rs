//! Safe core of `libobs/obs-nix-wayland-keys.c.h`: the keysym tables and the
//! keymap and focused-key rules of the Wayland hotkey platform
//! (`obs-nix-wayland.c`, Linux and FreeBSD).
//!
//! The Wayland hotkey platform (still C) asks this code:
//! - [`from_keysym`] (`obs_key_from_virtual_key`, and the keymap rows): the
//!   obs key of an XKB keysym;
//! - [`to_keysym`] (`obs_key_to_virtual_key`, and the GlobalShortcuts
//!   portal's trigger names): the XKB keysym of an obs key;
//! - [`keymap_row`], for each keycode of every keymap the compositor sends
//!   and on each layout change (`load_keymap_data()`): the first keysym of
//!   each shift level, and the keycode of each key, the first keycode
//!   winning;
//! - [`focused`], on each key event while OBS has keyboard focus
//!   (`set_focused_key()`): which key the hotkey thread is told is pressed
//!   or released.
//!
//! It reads and writes the caller's arrays and returns keys (`obs_key_t` as
//! `i32`), so the C keeps every xkbcommon and Wayland call, the arrays and
//! the hotkey state. A keymap row's keysyms come from a closure, called once
//! per level, in order, only for a keycode in range, so the shim asks
//! xkbcommon for them where the C did.

pub mod keysyms;

pub use crate::name_map::OBS_KEY_LAST_VALUE;

/// `MAX_KEYCODES`: X11 only supports 256 scancodes.
pub const MAX_KEYCODES: usize = 256;
/// `MAX_SHIFT_LEVELS`: X11 keymaps only have 4 shift levels.
pub const MAX_SHIFT_LEVELS: usize = 4;

/// `obs_nix_wayland_key_from_virtual_key`: the obs key of keysym `sym`,
/// `OBS_KEY_NONE` for one the table lacks.
pub fn from_keysym(sym: i32) -> i32 {
    let _ = sym;
    todo!()
}

/// `obs_nix_wayland_key_to_virtual_key`: the keysym of obs key `key`, 0 for
/// one the table lacks.
pub fn to_keysym(key: i32) -> u32 {
    let _ = key;
    todo!()
}

/// `load_keymap_data()`'s rule for one keycode: nothing for a keycode at or
/// above [`MAX_KEYCODES`]; else for each of the [`MAX_SHIFT_LEVELS`] levels,
/// `level_sym(level)` is its first keysym when it has at least one, which is
/// stored in `key_to_sym[level][keycode]`, and `keycode` in `obs_to_key` at
/// that keysym's key unless one is stored there (0 is none).
pub fn keymap_row(
    keycode: u32,
    level_sym: impl FnMut(u32) -> Option<u32>,
    obs_to_key: &mut [u32],
    key_to_sym: &mut [[u32; MAX_KEYCODES]],
) {
    let _ = (keycode, level_sym, obs_to_key, key_to_sym);
    todo!()
}

/// `set_focused_key()`'s rule for a focused key event of evdev scancode
/// `scancode` (a press when `pressed`): the key the hotkey state is told
/// about, or `None`.
///
/// Its keycode is `scancode + 8` (wrapping as a `u32`), none at or above
/// [`MAX_KEYCODES`]. A press reports the key of the keycode's level-0 keysym
/// in `key_to_sym0` (none for keysym 0) and records it in
/// `pressed_as[keycode]`; a release reports the key recorded for it, and
/// clears it, or nothing when none is recorded (`OBS_KEY_NONE`).
///
/// Intentional difference from C, which looks a release up again: a key
/// released after a layout change is released as the key it was pressed
/// as, not as what the new layout puts on its keycode.
pub fn focused(
    scancode: u32,
    pressed: bool,
    key_to_sym0: &[u32],
    pressed_as: &mut [i32],
) -> Option<i32> {
    let _ = (scancode, pressed, key_to_sym0, pressed_as);
    todo!()
}
