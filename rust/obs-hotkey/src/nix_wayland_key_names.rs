//! Safe core of `libobs/obs-nix-wayland-key-names.c.h`: the key names of the
//! Wayland hotkey platform (`obs-nix-wayland.c`, Linux and FreeBSD).
//!
//! The Wayland hotkey platform (still C) asks [`route`], from
//! `obs_key_to_str()`, how to name a key: a mouse button or numpad digit by
//! number, a fixed key by its default name, a function key by number, or
//! through the keymap's level-0 keysym. The table is the X11 platform's
//! ([`crate::nix_x11_hotkeys::route`]).
//!
//! The C keeps the translations, the keymap reads, every string and every
//! xkbcommon call. It keeps no state, takes no lock, allocates nothing and
//! logs nothing.
//!
//! Characterized and kept: F1 to F35 never look up a translation, unlike
//! every other named key.

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

/// How `obs_key_to_str()` names a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    /// Mouse button `n` (1 to 29): its translation, else `Mouse n`.
    Mouse(i32),
    /// Numpad digit `n` (0 to 9): its translation, else `Numpad n`.
    Numpad(i32),
    /// Its translation, with this default name.
    Fixed(&'static CStr),
    /// Function key `n` (1 to 35): `Fn`, with no translation.
    Function(i32),
    /// The UTF-8 of the key's level-0 keysym in the keymap, else its key
    /// name.
    Keysym,
}

/// How to name `key` (`obs_key_t`): the tests of
/// `obs_nix_wayland_key_to_str()`, in its order (mouse buttons, numpad
/// digits, fixed keys, function keys, then the keysym).
#[expect(clippy::todo, reason = "RED: the port comes next")]
pub fn route(key: i32) -> Route {
    let _ = key;
    todo!()
}
