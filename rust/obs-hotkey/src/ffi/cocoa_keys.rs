//! C ABI for `libobs/obs-cocoa-keys.c.h`, declared in
//! `libobs/obs-cocoa-keys.h`.
//!
//! The macOS hotkey platform (`obs-cocoa.m`) calls these inside libobs. The
//! C file has no `EXPORT`, so the symbols stay hidden (`rust-hidden.txt`).
//! Elsewhere the shims are built but unused, and hidden all the same.
//!
//! The table is the platform's `virtual_keys`: `OBS_KEY_LAST_VALUE`
//! entries of `obs_key_code_t` ([`KeyCode`]), only read. `obs_key_t` is a C
//! enum, passed as `int`; an event's flags (`CGEventFlags`) and the pressed
//! mouse buttons (`NSUInteger`) are 64 bits, as `obs-cocoa.m` checks.
//!
//! Intentional differences from C:
//! - `obs_cocoa_keys_to_virtual_key` returns `INVALID_KEY` for a key outside
//!   `0 .. OBS_KEY_LAST_VALUE - 1`; C reads past the table;
//! - `obs_cocoa_keys_mouse_pressed` tests bit `key - OBS_KEY_MOUSE1`; C
//!   shifts an `int` by `key - 1`, which is undefined;
//! - where C has undefined behavior, a null table reads as empty (C
//!   dereferences it; libobs always passes `virtual_keys`).

use std::ffi::c_int;

use crate::cocoa_keys::{self as core, KeyCode, OBS_KEY_LAST_VALUE};

/// The table at `table`, or none for a null pointer.
///
/// # Safety
///
/// A non-null `table` points to `OBS_KEY_LAST_VALUE` readable, initialized
/// entries that stay unchanged for `'a`.
unsafe fn table<'a>(table: *const KeyCode) -> &'a [KeyCode] {
    if table.is_null() {
        return &[];
    }
    // SAFETY: the caller's guarantee.
    unsafe { std::slice::from_raw_parts(table, OBS_KEY_LAST_VALUE) }
}

/// `obs_cocoa_keys_modifier()`: whether the modifier `key` is down in an
/// event's `flags`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_cocoa_keys_modifier(flags: u64, key: c_int) -> bool {
    core::modifier(flags, key)
}

/// `obs_cocoa_keys_to_virtual_key()`: `key`'s virtual key code in `table`,
/// or `INVALID_KEY`.
///
/// # Safety
///
/// `table` is null or points to `OBS_KEY_LAST_VALUE` entries.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_cocoa_keys_to_virtual_key(table: *const KeyCode, key: c_int) -> c_int {
    // SAFETY: the caller's guarantee.
    core::to_virtual(unsafe { self::table(table) }, key)
}

/// `obs_cocoa_keys_from_virtual_key()`: the first key in `table` with
/// `code`, or `OBS_KEY_NONE`.
///
/// # Safety
///
/// `table` is null or points to `OBS_KEY_LAST_VALUE` entries.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_cocoa_keys_from_virtual_key(
    table: *const KeyCode,
    code: c_int,
) -> c_int {
    // SAFETY: the caller's guarantee.
    core::from_virtual(unsafe { self::table(table) }, code)
}

/// `obs_cocoa_keys_is_mouse()`: whether `key` is a mouse button.
#[unsafe(no_mangle)]
pub extern "C" fn obs_cocoa_keys_is_mouse(key: c_int) -> bool {
    core::is_mouse(key)
}

/// `obs_cocoa_keys_mouse_pressed()`: whether the mouse key's button is down
/// in `buttons`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_cocoa_keys_mouse_pressed(key: c_int, buttons: u64) -> bool {
    core::mouse_pressed(key, buttons)
}

/// `obs_cocoa_keys_in_range()`: whether `key` indexes the key state.
#[unsafe(no_mangle)]
pub extern "C" fn obs_cocoa_keys_in_range(key: c_int) -> bool {
    core::in_range(key)
}
