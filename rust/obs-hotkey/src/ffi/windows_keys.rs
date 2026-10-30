//! C ABI for `libobs/obs-windows-keys.c.h`, declared in
//! `libobs/obs-windows-keys.h`.
//!
//! The Windows hotkey platform (`obs-windows.c`) calls these inside libobs.
//! The C file has no `EXPORT`, so the symbols stay hidden (`rust-hidden.txt`;
//! MSVC exports only what `rust-exports.d` lists). Elsewhere the shims are
//! built but unused, and hidden all the same.
//!
//! The table is the platform's `vk_codes`: `OBS_KEY_LAST_VALUE` `int`s,
//! only read, passed in for `obs_windows_keys_from_virtual_key` and asked
//! for through the caller's getter by `obs_windows_keys_to_virtual_key`
//! (only for a key other than Meta, as the C reads it). `obs_key_t` and
//! `enum obs_windows_keys_name` are C enums, passed as `int`; a scan code
//! is a `UINT`, 32 bits, as `obs-windows.c` checks.
//!
//! Intentional differences from C:
//! - `obs_windows_keys_to_virtual_key` returns 0 for a key outside
//!   `0 .. OBS_KEY_LAST_VALUE - 1`; C indexes the table with it;
//! - where C has undefined behavior, a null table or getter reads as an
//!   empty table, a null `number` is not written and a null `keys` gets
//!   no keys (C dereferences them; libobs always passes them).

use std::ffi::c_int;

use crate::windows_keys::{self as core, COMBINATION_MAX, Name, OBS_KEY_LAST_VALUE};

/// `OBS_WINDOWS_KEYS_NAME_NONE`.
pub const OBS_WINDOWS_KEYS_NAME_NONE: c_int = 0;
/// `OBS_WINDOWS_KEYS_NAME_F`.
pub const OBS_WINDOWS_KEYS_NAME_F: c_int = 1;
/// `OBS_WINDOWS_KEYS_NAME_MOUSE`.
pub const OBS_WINDOWS_KEYS_NAME_MOUSE: c_int = 2;
/// `OBS_WINDOWS_KEYS_NAME_PAUSE`.
pub const OBS_WINDOWS_KEYS_NAME_PAUSE: c_int = 3;
/// `OBS_WINDOWS_KEYS_NAME_META`.
pub const OBS_WINDOWS_KEYS_NAME_META: c_int = 4;
/// `OBS_WINDOWS_KEYS_NAME_SYSTEM`.
pub const OBS_WINDOWS_KEYS_NAME_SYSTEM: c_int = 5;

/// The table at `vk_codes`, or none for a null pointer.
///
/// # Safety
///
/// A non-null `vk_codes` points to `OBS_KEY_LAST_VALUE` readable,
/// initialized `int`s that stay unchanged for `'a`.
unsafe fn table<'a>(vk_codes: *const c_int) -> &'a [c_int] {
    if vk_codes.is_null() {
        return &[];
    }
    // SAFETY: the caller's guarantee.
    unsafe { std::slice::from_raw_parts(vk_codes, OBS_KEY_LAST_VALUE) }
}

/// `obs_windows_keys_virtual_key()`: the virtual key of `key`, or 0.
#[unsafe(no_mangle)]
pub extern "C" fn obs_windows_keys_virtual_key(key: c_int) -> c_int {
    core::virtual_key(key)
}

/// `obs_windows_keys_either_win()`: whether `key` is down when either
/// Windows key is.
#[unsafe(no_mangle)]
pub extern "C" fn obs_windows_keys_either_win(key: c_int) -> bool {
    core::either_win(key)
}

/// `obs_windows_keys_name()`: `key`'s name route; writes `*number` only for
/// `OBS_WINDOWS_KEYS_NAME_F` and `OBS_WINDOWS_KEYS_NAME_MOUSE`.
///
/// # Safety
///
/// `number` is null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_windows_keys_name(key: c_int, number: *mut c_int) -> c_int {
    let (route, n) = match core::name(key) {
        Name::None => (OBS_WINDOWS_KEYS_NAME_NONE, None),
        Name::F(n) => (OBS_WINDOWS_KEYS_NAME_F, Some(n)),
        Name::Mouse(n) => (OBS_WINDOWS_KEYS_NAME_MOUSE, Some(n)),
        Name::Pause => (OBS_WINDOWS_KEYS_NAME_PAUSE, None),
        Name::Meta => (OBS_WINDOWS_KEYS_NAME_META, None),
        Name::System => (OBS_WINDOWS_KEYS_NAME_SYSTEM, None),
    };
    if let Some(n) = n
        && !number.is_null()
    {
        // SAFETY: the caller guarantees a non-null `number` is writable.
        unsafe { number.write(n) };
    }
    route
}

/// `obs_windows_keys_scan_code()`: the `GetKeyNameTextW()` key code of
/// virtual key `vk` with scan code `mapped`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_windows_keys_scan_code(vk: c_int, mapped: u32) -> u32 {
    core::scan_code(vk, mapped)
}

/// `obs_windows_keys_system_name()`: whether `key`'s name is asked of the
/// system.
#[unsafe(no_mangle)]
pub extern "C" fn obs_windows_keys_system_name(key: c_int, scan_code: u32) -> bool {
    core::system_name(key, scan_code)
}

/// `obs_windows_keys_from_virtual_key()`: the first key whose entry in
/// `vk_codes` is `code`, or `OBS_KEY_NONE`.
///
/// # Safety
///
/// `vk_codes` is null or points to `OBS_KEY_LAST_VALUE` `int`s.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_windows_keys_from_virtual_key(
    vk_codes: *const c_int,
    code: c_int,
) -> c_int {
    // SAFETY: the caller's guarantee.
    core::from_virtual(unsafe { table(vk_codes) }, code)
}

/// `obs_windows_keys_table_fn`: the `vk_codes` table's getter.
pub type TableFn = Option<unsafe extern "C" fn() -> *const c_int>;

/// `obs_windows_keys_to_virtual_key()`: `VK_LWIN` for Meta, else `key`'s
/// entry in the table `vk_codes` returns, or 0 outside it.
///
/// # Safety
///
/// `vk_codes` is null or returns null or a pointer to `OBS_KEY_LAST_VALUE`
/// `int`s.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_windows_keys_to_virtual_key(key: c_int, vk_codes: TableFn) -> c_int {
    core::to_virtual(key, || match vk_codes {
        // SAFETY: the caller's guarantee; called only when the core asks.
        Some(vk_codes) => unsafe { table(vk_codes()) },
        None => &[],
    })
}

/// `obs_windows_keys_combination()`: writes the keys a combination names to
/// `keys` and returns how many.
///
/// # Safety
///
/// `keys` is null or points to `OBS_WINDOWS_KEYS_COMBINATION_MAX` writable
/// `obs_key_t`s.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_windows_keys_combination(
    modifiers: u32,
    key: c_int,
    keys: *mut c_int,
) -> usize {
    let combination = core::combination(modifiers, key);
    let named = combination.keys();
    if keys.is_null() {
        return 0;
    }
    // SAFETY: the caller guarantees `COMBINATION_MAX` writable keys; a
    // combination holds at most that many.
    let out = unsafe { std::slice::from_raw_parts_mut(keys, COMBINATION_MAX) };
    for (slot, &named) in out.iter_mut().zip(named) {
        *slot = named;
    }
    named.len()
}
