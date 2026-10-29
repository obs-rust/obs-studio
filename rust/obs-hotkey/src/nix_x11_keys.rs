//! Safe core of `libobs/obs-nix-x11-keys.c.h`: the X11 key map lookups of
//! the X11 hotkey platform (`obs-nix-x11.c`, Linux and FreeBSD).
//!
//! The X11 hotkey platform (still C) asks this code:
//! - [`row_key`], for each keycode's row of the server's keyboard map at
//!   init (`fill_keycodes()`): whether the keycode is a super key's, which
//!   key it produces, or neither; [`key_from_base`] is its keysym lookup;
//! - [`any_pressed`], on each hotkey poll (`key_pressed()`): whether a key
//!   is down in the QueryKeymap reply's bitmap;
//! - [`from_virtual`] (`obs_key_from_virtual_key`): the key of a keysym,
//!   searched one column (group and level) at a time across all keycodes,
//!   then [`key_from_keycode`];
//! - [`to_virtual`] (`obs_key_to_virtual_key`): the keysym of a key.
//!
//! It reads the caller's tables and returns keys (`obs_key_t` as `i32`), so
//! the C keeps the keysym table, the keycode lists, the keyboard map copy
//! and every X call. A keycode list is any `AsRef<[u8]>` (a slice, or the
//! C darray in the shim).
//!
//! Intentional difference from C: [`to_virtual`] gives 0 for a key outside
//! the table; C indexes the table with any key.
//!
//! Every read goes through a bounds check, so nothing past the slices the
//! caller passes is read: the shim passes the table and the keyboard map
//! with the lengths the C computed.
//!
//! An input the C read only under a condition comes as a closure, called
//! only under that condition (the super keycodes for Meta, a key's keycodes
//! for any other key, the map's size past the search's gate, the minimum
//! keycode and the lists once a symbol is found, the base table for any key
//! but Meta), so the shim reads the platform's fields where the C did.

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

use crate::name_map::{KEY_NONE, OBS_KEY_ALT, OBS_KEY_CONTROL, OBS_KEY_META, OBS_KEY_SHIFT};

pub use crate::name_map::OBS_KEY_LAST_VALUE;

/// `XK_Shift_R`.
pub const SHIFT_R: u32 = 0xffe2;
/// `XK_Control_R`.
pub const CONTROL_R: u32 = 0xffe4;
/// `XK_Alt_R`.
pub const ALT_R: u32 = 0xffea;
/// `XK_Super_L`.
pub const SUPER_L: u32 = 0xffeb;
/// `XK_Super_R`.
pub const SUPER_R: u32 = 0xffec;

/// The size of a QueryKeymap reply's key bitmap: one bit per keycode.
pub const KEYMAP_LEN: usize = 32;

/// What `fill_keycodes()` does with one keycode's row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    /// No symbol of the row is wanted.
    None,
    /// The keycode is the left super key's.
    SuperL,
    /// The keycode is the right super key's.
    SuperR,
    /// The keycode produces this key.
    Key(i32),
}

/// `key_from_base_keysym()`: the first key whose base keysym in `table` is
/// `code`; else [`SHIFT_R`], [`CONTROL_R`] and [`ALT_R`] give Shift,
/// Control and Alt; else `OBS_KEY_NONE`.
pub fn key_from_base(table: &[u32], code: u32) -> i32 {
    if let Some(key) = table.iter().position(|&sym| sym == code) {
        return key_of(key);
    }

    match code {
        SHIFT_R => OBS_KEY_SHIFT,
        CONTROL_R => OBS_KEY_CONTROL,
        ALT_R => OBS_KEY_ALT,
        _ => KEY_NONE,
    }
}

/// A table position as an `obs_key_t`. The tables have
/// `OBS_KEY_LAST_VALUE` entries, so every position fits; a longer one's
/// positions past `i32::MAX` give `OBS_KEY_NONE`.
fn key_of(position: usize) -> i32 {
    i32::try_from(position).unwrap_or(KEY_NONE)
}

/// `fill_keycodes()`'s rule for one keycode's `row`: the first non-zero
/// symbol that is [`SUPER_L`], [`SUPER_R`] or has a key in `table`; a zero
/// symbol ends the row.
pub fn row_key(row: &[u32], table: &[u32]) -> Row {
    for &sym in row {
        if sym == 0 {
            break;
        }

        if sym == SUPER_L {
            return Row::SuperL;
        } else if sym == SUPER_R {
            return Row::SuperR;
        }

        let key = key_from_base(table, sym);
        if key != KEY_NONE {
            return Row::Key(key);
        }
    }

    Row::None
}

/// `key_pressed()`'s decision: whether `key` is down in `keys`, a
/// QueryKeymap reply's bitmap (`None` for no reply: nothing is down).
/// `OBS_KEY_META` checks `super_l()`, then `super_r()` only when the left
/// one is not down; any other key each of its `codes()`. Neither super
/// keycode is asked for any other key, nor `codes` for Meta.
pub fn any_pressed<C: AsRef<[u8]>>(
    keys: Option<&[u8; KEYMAP_LEN]>,
    key: i32,
    codes: impl FnOnce() -> C,
    super_l: impl FnOnce() -> u8,
    super_r: impl FnOnce() -> u8,
) -> bool {
    let down = |code: u8| keycode_down(keys, code);

    if key == OBS_KEY_META {
        down(super_l()) || down(super_r())
    } else {
        codes().as_ref().iter().any(|&code| down(code))
    }
}

/// `obs_nix_x11_keycode_pressed()`: keycode `code`'s bit in the bitmap.
fn keycode_down(keys: Option<&[u8; KEYMAP_LEN]>, code: u8) -> bool {
    keys.and_then(|keys| keys.get(usize::from(code / 8)))
        .is_some_and(|&byte| byte & (1 << (code % 8)) != 0)
}

/// `key_from_keycode()`: the first key whose list in `lists` holds `code`,
/// or `OBS_KEY_NONE`.
pub fn key_from_keycode<L: AsRef<[u8]>>(lists: &[L], code: u8) -> i32 {
    lists
        .iter()
        .position(|list| list.as_ref().contains(&code))
        .map_or(KEY_NONE, key_of)
}

/// `obs_nix_x11_key_from_virtual_key()`: the key of keysym `sym` in the
/// keyboard map `keysyms` (`syms_per_code` symbols per keycode from
/// `min_keycode` on), searched one column at a time across all keycodes,
/// so the keycode that has it unshifted in the first group wins; then
/// [`key_from_keycode`] of the first keycode found. `OBS_KEY_NONE` for
/// `sym` 0 or `syms_per_code` 0 or less, before `keysyms()` is asked;
/// `min_keycode()` and `lists()` are asked only once the symbol is found.
pub fn from_virtual<M, Ls, L>(
    sym: i32,
    syms_per_code: i32,
    keysyms: impl FnOnce() -> M,
    min_keycode: impl FnOnce() -> u8,
    lists: impl FnOnce() -> Ls,
) -> i32
where
    M: AsRef<[u32]>,
    Ls: AsRef<[L]>,
    L: AsRef<[u8]>,
{
    let Ok(per) = usize::try_from(syms_per_code) else {
        return KEY_NONE;
    };
    if sym == 0 || per == 0 {
        return KEY_NONE;
    }

    // The keysym's 32 bits, as C compares it with an xcb_keysym_t.
    let sym = u32::from_ne_bytes(sym.to_ne_bytes());
    let keysyms = keysyms();
    // Whole rows only, as C's num_keysyms / syms_per_code.
    let rows = keysyms.as_ref().chunks_exact(per);

    for col in 0..per {
        for (row, symbols) in rows.clone().enumerate() {
            if symbols.get(col) == Some(&sym) {
                let code = keycode(row, min_keycode());
                return key_from_keycode(lists().as_ref(), code);
            }
        }
    }

    KEY_NONE
}

/// The keycode of the map's `row`: `min_keycode` plus the row, as a
/// `uint8_t` (C's `(xcb_keycode_t)(i + min_keycode)`), so it wraps.
fn keycode(row: usize, min_keycode: u8) -> u8 {
    let low = row.to_le_bytes().first().copied().unwrap_or(0);
    low.wrapping_add(min_keycode)
}

/// `obs_nix_x11_key_to_virtual_key()`: [`SUPER_L`] for `OBS_KEY_META`; any
/// other key's base keysym in `table()`, asked only then, as an `int`; 0
/// for a key outside the table.
pub fn to_virtual<T: AsRef<[u32]>>(key: i32, table: impl FnOnce() -> T) -> i32 {
    if key == OBS_KEY_META {
        return i32::from_ne_bytes(SUPER_L.to_ne_bytes());
    }

    let table = table();
    usize::try_from(key)
        .ok()
        .and_then(|key| table.as_ref().get(key))
        .map_or(0, |&sym| i32::from_ne_bytes(sym.to_ne_bytes()))
}
