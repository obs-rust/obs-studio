//! C ABI for `libobs/obs-nix-x11-hotkeys.c.h`, declared in
//! `libobs/obs-nix-x11-hotkeys.h`.
//!
//! The X11 hotkey platform (`obs-nix-x11.c`) calls these inside libobs on
//! Linux and FreeBSD. The C file has no `EXPORT`, so the symbols stay hidden
//! (`rust-hidden.txt`). Elsewhere the shims are built but unused, and hidden
//! all the same.
//!
//! Both functions return their C enum (`int`) and write their out-parameters
//! only where the C did: `number` for the mouse, numpad and function routes,
//! `name` (a static string) for the fixed one, `slot` for a raw press or
//! release. The event type and the detail cross by pointer and are read only
//! where the C read them. `obs_key_t` is a C enum, passed as `int`.
//!
//! Intentional differences from C, only where C has undefined behavior: a
//! null out-parameter is not written, and a null event type or detail reads
//! as 0 (C dereferences them).

use std::ffi::{c_char, c_int};

use crate::nix_x11_hotkeys::{self as core, RawButton, Route};

/// `enum obs_nix_x11_hotkeys_route`.
const ROUTE_NONE: c_int = 0;
const ROUTE_MOUSE: c_int = 1;
const ROUTE_NUMPAD: c_int = 2;
const ROUTE_FIXED: c_int = 3;
const ROUTE_FUNCTION: c_int = 4;
const ROUTE_KEYCODES: c_int = 5;

/// `enum obs_nix_x11_hotkeys_button`.
const BUTTON_IGNORE: c_int = 0;
const BUTTON_PRESS: c_int = 1;
const BUTTON_RELEASE: c_int = 2;
const BUTTON_UNSUPPORTED: c_int = 3;

/// Stores `value` at `out` unless it is null.
///
/// # Safety
///
/// A non-null `out` points to a writable `T`.
unsafe fn store<T>(out: *mut T, value: T) {
    // SAFETY: the caller's guarantee.
    if let Some(out) = unsafe { out.as_mut() } {
        *out = value;
    }
}

/// # Safety
///
/// `number` is null or points to a writable `int`; `name` is null or points
/// to a writable `const char *`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_nix_x11_hotkeys_route(
    key: c_int,
    number: *mut c_int,
    name: *mut *const c_char,
) -> c_int {
    let (code, number_value, name_value) = match core::route(key) {
        Route::None => (ROUTE_NONE, None, None),
        Route::Mouse(n) => (ROUTE_MOUSE, Some(n), None),
        Route::Numpad(n) => (ROUTE_NUMPAD, Some(n), None),
        Route::Fixed(default) => (ROUTE_FIXED, None, Some(default.as_ptr())),
        Route::Function(n) => (ROUTE_FUNCTION, Some(n), None),
        Route::Keycodes => (ROUTE_KEYCODES, None, None),
    };
    if let Some(n) = number_value {
        // SAFETY: the caller's guarantee.
        unsafe { store(number, n) };
    }
    if let Some(default) = name_value {
        // SAFETY: the caller's guarantee.
        unsafe { store(name, default) };
    }
    code
}

/// # Safety
///
/// `event_type` and `detail` are null or point to a readable `uint16_t` and
/// `uint32_t`; `slot` is null or points to a writable `uint32_t`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_nix_x11_hotkeys_raw_button(
    response_type: u8,
    event_type: *const u16,
    detail: *const u32,
    slot: *mut u32,
) -> c_int {
    // SAFETY: the caller's guarantees.
    let event_type = || unsafe { event_type.as_ref() }.copied().unwrap_or(0);
    // SAFETY: the caller's guarantees.
    let detail = || unsafe { detail.as_ref() }.copied().unwrap_or(0);
    let (button, index) = match core::raw_button(response_type, event_type, detail) {
        RawButton::Ignore => return BUTTON_IGNORE,
        RawButton::Unsupported => return BUTTON_UNSUPPORTED,
        RawButton::Press(index) => (BUTTON_PRESS, index),
        RawButton::Release(index) => (BUTTON_RELEASE, index),
    };
    // The core's slots are below MOUSE_LEN, so they fit.
    let index = u32::try_from(index).unwrap_or(u32::MAX);
    // SAFETY: the caller's guarantee.
    unsafe { store(slot, index) };
    button
}
