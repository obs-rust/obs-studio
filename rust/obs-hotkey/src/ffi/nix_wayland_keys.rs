//! C ABI for `libobs/obs-nix-wayland-keys.c.h`, declared in
//! `libobs/obs-nix-wayland-keys.h`.
//!
//! The Wayland hotkey platform (`obs-nix-wayland.c`) calls these inside
//! libobs on Linux and FreeBSD with `ENABLE_WAYLAND`. The C file has no
//! `EXPORT`, so the symbols stay hidden (`rust-hidden.txt`). Elsewhere the
//! shims are built but unused, and hidden all the same.
//!
//! The arrays are the platform's: `obs_to_key` has `OBS_KEY_LAST_VALUE`
//! keycodes, `key_to_sym` `MAX_SHIFT_LEVELS` rows of `MAX_KEYCODES` keysyms,
//! and `pressed_as` `MAX_KEYCODES` keys. A keymap row's keysyms come from the
//! caller's callback (`xkb_keymap_key_get_syms_by_level()`), called once per
//! level, in order, only for a keycode in range; its first keysym is read
//! only when it reports at least one. `obs_key_t` is a C enum, passed as
//! `int`.
//!
//! Intentional differences from C:
//! - `obs_nix_wayland_keys_focused` releases the key the keycode was pressed
//!   as, from `pressed_as`, and records each press there; C looks a release
//!   up again on the current layout and leaves the record alone;
//! - where C has undefined behavior, a null array reads as empty (nothing
//!   is read or written) and a null callback as a level with no keysym.

use std::ffi::{c_int, c_void};

use crate::nix_wayland_keys::{self as core, MAX_KEYCODES, MAX_SHIFT_LEVELS, OBS_KEY_LAST_VALUE};

/// `obs_nix_wayland_keys_syms_fn`.
pub type SymsFn =
    unsafe extern "C" fn(data: *mut c_void, level: u32, syms: *mut *const u32) -> c_int;

/// `len` items at `ptr`, or none for a null pointer.
///
/// # Safety
///
/// A non-null `ptr` points to `len` initialized items that stay unchanged
/// for `'a`.
unsafe fn items<'a, T>(ptr: *const T, len: usize) -> &'a [T] {
    if ptr.is_null() {
        return &[];
    }
    // SAFETY: the caller's guarantee.
    unsafe { std::slice::from_raw_parts(ptr, len) }
}

/// `len` items at `ptr`, or none for a null pointer.
///
/// # Safety
///
/// A non-null `ptr` points to `len` initialized items that nothing else
/// accesses for `'a`.
unsafe fn items_mut<'a, T>(ptr: *mut T, len: usize) -> &'a mut [T] {
    if ptr.is_null() {
        return &mut [];
    }
    // SAFETY: the caller's guarantee.
    unsafe { std::slice::from_raw_parts_mut(ptr, len) }
}

#[unsafe(no_mangle)]
pub extern "C" fn obs_nix_wayland_keys_from_keysym(sym: c_int) -> c_int {
    core::from_keysym(sym)
}

#[unsafe(no_mangle)]
pub extern "C" fn obs_nix_wayland_keys_to_keysym(key: c_int) -> c_int {
    // Every keysym of the table is below 2^31.
    c_int::try_from(core::to_keysym(key)).unwrap_or(0)
}

/// # Safety
///
/// `level_syms` is null or safe to call with `data`, and sets `*syms` to
/// its count of readable keysyms when that count is at least 1;
/// `obs_to_key` points to `OBS_KEY_LAST_VALUE` keycodes and `key_to_sym` to
/// `MAX_SHIFT_LEVELS` rows of `MAX_KEYCODES` keysyms, each null or writable
/// and not otherwise accessed during the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_nix_wayland_keys_row(
    keycode: u32,
    level_syms: Option<SymsFn>,
    data: *mut c_void,
    obs_to_key: *mut u32,
    key_to_sym: *mut [u32; MAX_KEYCODES],
) {
    // SAFETY: the caller's guarantee.
    let obs_to_key = unsafe { items_mut(obs_to_key, OBS_KEY_LAST_VALUE) };
    // SAFETY: the caller's guarantee.
    let key_to_sym = unsafe { items_mut(key_to_sym, MAX_SHIFT_LEVELS) };
    core::keymap_row(
        keycode,
        |level| {
            let level_syms = level_syms?;
            let mut syms: *const u32 = std::ptr::null();
            // SAFETY: the caller's guarantee.
            let count = unsafe { level_syms(data, level, &mut syms) };
            if count < 1 || syms.is_null() {
                return None;
            }
            // SAFETY: a count of at least 1 means `syms` holds a keysym.
            Some(unsafe { *syms })
        },
        obs_to_key,
        key_to_sym,
    );
}

/// # Safety
///
/// `key_to_sym0` points to `MAX_KEYCODES` keysyms and `pressed_as` to
/// `MAX_KEYCODES` keys, each null or valid and not otherwise accessed during
/// the call; `key` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_nix_wayland_keys_focused(
    scancode: u32,
    pressed: bool,
    key_to_sym0: *const u32,
    pressed_as: *mut c_int,
    key: *mut c_int,
) -> bool {
    // SAFETY: the caller's guarantee.
    let key_to_sym0 = unsafe { items(key_to_sym0, MAX_KEYCODES) };
    // SAFETY: the caller's guarantee.
    let pressed_as = unsafe { items_mut(pressed_as, MAX_KEYCODES) };
    let Some(reported) = core::focused(scancode, pressed, key_to_sym0, pressed_as) else {
        return false;
    };
    if !key.is_null() {
        // SAFETY: the caller's guarantee.
        unsafe { key.write(reported) };
    }
    true
}
