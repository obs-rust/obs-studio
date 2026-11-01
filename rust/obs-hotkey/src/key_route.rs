//! The key name table that the X11 and Wayland hotkey platforms share:
//! `obs_nix_x11_key_to_str()` and `obs_nix_wayland_key_to_str()` test the
//! same keys in the same order, with the same default names, and differ
//! only in how they name every other key. [`crate::nix_x11_hotkeys::route`]
//! and [`crate::nix_wayland_key_names::route`] map [`Named::Other`] to
//! their own route.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::as_conversions,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable
)]

use std::ffi::CStr;

use crate::name_map as k;

/// How the table names a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Named {
    /// Mouse button `n` (1 to 29).
    Mouse(i32),
    /// Numpad digit `n` (0 to 9).
    Numpad(i32),
    /// A fixed key, with this default name.
    Fixed(&'static CStr),
    /// Function key `n` (1 to 35).
    Function(i32),
    /// None of those: the platform names it.
    Other,
}

/// The tests of both platforms' `key_to_str`, in their order: mouse
/// buttons, numpad digits, fixed keys, then function keys.
pub(crate) fn named(key: i32) -> Named {
    match key {
        k::OBS_KEY_MOUSE1..=k::OBS_KEY_MOUSE29 => {
            Named::Mouse(key.wrapping_sub(k::OBS_KEY_MOUSE1).wrapping_add(1))
        }
        k::OBS_KEY_NUM0..=k::OBS_KEY_NUM9 => Named::Numpad(key.wrapping_sub(k::OBS_KEY_NUM0)),
        k::OBS_KEY_INSERT => Named::Fixed(c"Insert"),
        k::OBS_KEY_DELETE => Named::Fixed(c"Delete"),
        k::OBS_KEY_HOME => Named::Fixed(c"Home"),
        k::OBS_KEY_END => Named::Fixed(c"End"),
        k::OBS_KEY_PAGEUP => Named::Fixed(c"Page Up"),
        k::OBS_KEY_PAGEDOWN => Named::Fixed(c"Page Down"),
        k::OBS_KEY_NUMLOCK => Named::Fixed(c"Num Lock"),
        k::OBS_KEY_SCROLLLOCK => Named::Fixed(c"Scroll Lock"),
        k::OBS_KEY_CAPSLOCK => Named::Fixed(c"Caps Lock"),
        k::OBS_KEY_BACKSPACE => Named::Fixed(c"Backspace"),
        k::OBS_KEY_TAB => Named::Fixed(c"Tab"),
        k::OBS_KEY_PRINT => Named::Fixed(c"Print"),
        k::OBS_KEY_PAUSE => Named::Fixed(c"Pause"),
        k::OBS_KEY_LEFT => Named::Fixed(c"Left"),
        k::OBS_KEY_RIGHT => Named::Fixed(c"Right"),
        k::OBS_KEY_UP => Named::Fixed(c"Up"),
        k::OBS_KEY_DOWN => Named::Fixed(c"Down"),
        k::OBS_KEY_SHIFT => Named::Fixed(c"Shift"),
        k::OBS_KEY_ALT => Named::Fixed(c"Alt"),
        k::OBS_KEY_CONTROL => Named::Fixed(c"Control"),
        k::OBS_KEY_META => Named::Fixed(c"Super"),
        k::OBS_KEY_MENU => Named::Fixed(c"Menu"),
        k::OBS_KEY_NUMASTERISK => Named::Fixed(c"Numpad *"),
        k::OBS_KEY_NUMPLUS => Named::Fixed(c"Numpad +"),
        k::OBS_KEY_NUMMINUS => Named::Fixed(c"Numpad -"),
        k::OBS_KEY_NUMCOMMA => Named::Fixed(c"Numpad ,"),
        k::OBS_KEY_NUMPERIOD => Named::Fixed(c"Numpad ."),
        k::OBS_KEY_NUMSLASH => Named::Fixed(c"Numpad /"),
        k::OBS_KEY_SPACE => Named::Fixed(c"Space"),
        k::OBS_KEY_ESCAPE => Named::Fixed(c"Escape"),
        k::OBS_KEY_F1..=k::OBS_KEY_F35 => {
            Named::Function(key.wrapping_sub(k::OBS_KEY_F1).wrapping_add(1))
        }
        _ => Named::Other,
    }
}
