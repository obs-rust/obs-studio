//! Safe core of `libobs/obs-cocoa-keys.c.h`: the key map and pressed check
//! of the macOS hotkey platform (`obs-cocoa.m`).
//!
//! The macOS hotkey platform (still Objective-C) asks this code:
//! - [`modifier`], in the event tap's callback (`KeyboardEventProc()`):
//!   whether Shift, Alt (Option), Meta (Command) or Control is down in an
//!   event's flags; [`modifiers`] gives all four;
//! - [`to_virtual`] (`obs_key_to_virtual_key`): the virtual key code
//!   (`kVK_*`) of a key, or [`INVALID`];
//! - [`from_virtual`] (`obs_key_from_virtual_key`, also the callback's key
//!   down and key up): the key of a virtual key code;
//! - [`is_mouse`], [`mouse_pressed`] and [`in_range`], on each hotkey poll
//!   (`obs_hotkeys_platform_is_pressed()`): whether a key is a mouse button,
//!   whether its button is down in `[NSEvent pressedMouseButtons]`, and
//!   whether a key indexes the key state.
//!
//! It reads the caller's table and returns keys (`obs_key_t` as `i32`), so
//! the C keeps the `virtual_keys` table, the key state and its writes, and
//! every CGEvent and NSEvent call. It keeps no state, takes no lock and
//! allocates nothing.
//!
//! Intentional differences from C:
//! - [`to_virtual`] gives [`INVALID`] for a key outside the table; C reads
//!   past its end;
//! - [`mouse_pressed`] tests bit `key - OBS_KEY_MOUSE1` (MOUSE1 is the left
//!   button, bit 0); C shifts an `int` by `key - 1`, the key's enum value
//!   less one, which is past the width of an `int` for every mouse key, so
//!   undefined, and in practice tests an arbitrary bit or none.

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

use crate::name_map::KEY_NONE;

pub use crate::name_map::{
    OBS_KEY_ALT, OBS_KEY_CONTROL, OBS_KEY_LAST_VALUE, OBS_KEY_META, OBS_KEY_MOUSE1,
    OBS_KEY_MOUSE29, OBS_KEY_SHIFT,
};

/// `kCGEventFlagMaskShift`.
pub const FLAG_SHIFT: u64 = 0x0002_0000;
/// `kCGEventFlagMaskControl`.
pub const FLAG_CONTROL: u64 = 0x0004_0000;
/// `kCGEventFlagMaskAlternate` (Option).
pub const FLAG_ALTERNATE: u64 = 0x0008_0000;
/// `kCGEventFlagMaskCommand`.
pub const FLAG_COMMAND: u64 = 0x0010_0000;

/// `INVALID_KEY`: the virtual key of a key with none.
pub const INVALID: i32 = 0xFF;

/// `obs_key_code_t`: one entry of `virtual_keys`, a key's macOS virtual key
/// code, if it has one.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct KeyCode {
    /// The `kVK_*` code.
    pub code: i32,
    /// Whether the key has a code.
    pub is_valid: bool,
}

/// `KeyboardEventProc()`'s modifier map: whether `key` is down in `flags`.
/// [`OBS_KEY_SHIFT`], [`OBS_KEY_ALT`], [`OBS_KEY_META`] and
/// [`OBS_KEY_CONTROL`] read [`FLAG_SHIFT`], [`FLAG_ALTERNATE`],
/// [`FLAG_COMMAND`] and [`FLAG_CONTROL`]; any other key is not down.
pub fn modifier(flags: u64, key: i32) -> bool {
    let mask = match key {
        OBS_KEY_SHIFT => FLAG_SHIFT,
        OBS_KEY_ALT => FLAG_ALTERNATE,
        OBS_KEY_META => FLAG_COMMAND,
        OBS_KEY_CONTROL => FLAG_CONTROL,
        _ => return false,
    };
    flags & mask != 0
}

/// Shift, Alt, Meta and Control, in that order: [`modifier`] for each.
pub fn modifiers(flags: u64) -> [bool; 4] {
    [OBS_KEY_SHIFT, OBS_KEY_ALT, OBS_KEY_META, OBS_KEY_CONTROL].map(|key| modifier(flags, key))
}

/// `obs_key_to_virtual_key()`: the code of `key`'s entry in `table` when it
/// is valid; [`INVALID`] for an invalid entry or a key outside the table.
pub fn to_virtual(table: &[KeyCode], key: i32) -> i32 {
    usize::try_from(key)
        .ok()
        .and_then(|key| table.get(key))
        .filter(|entry| entry.is_valid)
        .map_or(INVALID, |entry| entry.code)
}

/// `obs_key_from_virtual_key()`: the first key whose valid entry in `table`
/// has `code`, or `OBS_KEY_NONE`.
pub fn from_virtual(table: &[KeyCode], code: i32) -> i32 {
    table
        .iter()
        .position(|entry| entry.is_valid && entry.code == code)
        .and_then(|key| i32::try_from(key).ok())
        .unwrap_or(KEY_NONE)
}

/// Whether `key` is a mouse button, [`OBS_KEY_MOUSE1`] to
/// [`OBS_KEY_MOUSE29`].
pub fn is_mouse(key: i32) -> bool {
    (OBS_KEY_MOUSE1..=OBS_KEY_MOUSE29).contains(&key)
}

/// The bit of a mouse key's button in the pressed mouse button mask:
/// `key - OBS_KEY_MOUSE1` (0 for the left button); `None` for any other
/// key.
pub fn mouse_bit(key: i32) -> Option<u32> {
    if !is_mouse(key) {
        return None;
    }
    key.checked_sub(OBS_KEY_MOUSE1)
        .and_then(|bit| u32::try_from(bit).ok())
}

/// Whether the mouse key's button is down in `buttons`
/// (`[NSEvent pressedMouseButtons]`): its [`mouse_bit`] is set. Never for
/// any other key.
pub fn mouse_pressed(key: i32, buttons: u64) -> bool {
    mouse_bit(key)
        .and_then(|bit| buttons.checked_shr(bit))
        .is_some_and(|rest| rest & 1 != 0)
}

/// Whether `key` indexes the key state: `0 .. OBS_KEY_LAST_VALUE`.
pub fn in_range(key: i32) -> bool {
    usize::try_from(key).is_ok_and(|key| key < OBS_KEY_LAST_VALUE)
}
