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
//! An input the C read only under a condition comes as a closure, called
//! only under that condition (the super keycodes for Meta, a key's keycodes
//! for any other key, the map's size past the search's gate, the minimum
//! keycode and the lists once a symbol is found, the base table for any key
//! but Meta), so the shim reads the platform's fields where the C did.

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
    let _ = (table, code);
    todo!()
}

/// `fill_keycodes()`'s rule for one keycode's `row`: the first non-zero
/// symbol that is [`SUPER_L`], [`SUPER_R`] or has a key in `table`; a zero
/// symbol ends the row.
pub fn row_key(row: &[u32], table: &[u32]) -> Row {
    let _ = (row, table);
    todo!()
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
    let _ = (keys, key, codes, super_l, super_r);
    todo!()
}

/// `key_from_keycode()`: the first key whose list in `lists` holds `code`,
/// or `OBS_KEY_NONE`.
pub fn key_from_keycode<L: AsRef<[u8]>>(lists: &[L], code: u8) -> i32 {
    let _ = (lists, code);
    todo!()
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
    let _ = (sym, keysyms, syms_per_code, min_keycode, lists);
    todo!()
}

/// `obs_nix_x11_key_to_virtual_key()`: [`SUPER_L`] for `OBS_KEY_META`; any
/// other key's base keysym in `table()`, asked only then, as an `int`; 0
/// for a key outside the table.
pub fn to_virtual<T: AsRef<[u32]>>(key: i32, table: impl FnOnce() -> T) -> i32 {
    let _ = (key, table);
    todo!()
}
