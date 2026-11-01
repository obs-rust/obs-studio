//! C ABI for `libobs/obs-nix-wayland-key-names.c.h`, declared in
//! `libobs/obs-nix-wayland-key-names.h`.
//!
//! The Wayland hotkey platform (`obs-nix-wayland.c`) calls it inside libobs
//! on Linux and FreeBSD. The C file has no `EXPORT`, so the symbol stays
//! hidden (`rust-hidden.d`). Elsewhere the shim is built but unused, and
//! hidden all the same.
//!
//! It returns its C enum (`int`) and writes its out-parameters only where
//! the C did: `number` for the mouse, numpad and function routes, `name` (a
//! static string) for the fixed one. `obs_key_t` is a C enum, passed as
//! `int`.
//!
//! Intentional difference from C, only where C has undefined behavior: a
//! null out-parameter is not written (C dereferences it).

use std::ffi::{c_char, c_int};

use crate::nix_wayland_key_names::{self as core, Route};

/// `enum obs_nix_wayland_key_names_route`. `NONE` (0) is never returned.
const ROUTE_MOUSE: c_int = 1;
const ROUTE_NUMPAD: c_int = 2;
const ROUTE_FIXED: c_int = 3;
const ROUTE_FUNCTION: c_int = 4;
const ROUTE_KEYSYM: c_int = 5;

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
pub unsafe extern "C" fn obs_nix_wayland_key_names_route(
    key: c_int,
    number: *mut c_int,
    name: *mut *const c_char,
) -> c_int {
    match core::route(key) {
        Route::Mouse(n) => {
            // SAFETY: the caller's guarantee.
            unsafe { store(number, n) };
            ROUTE_MOUSE
        }
        Route::Numpad(n) => {
            // SAFETY: the caller's guarantee.
            unsafe { store(number, n) };
            ROUTE_NUMPAD
        }
        Route::Fixed(default) => {
            // SAFETY: the caller's guarantee.
            unsafe { store(name, default.as_ptr()) };
            ROUTE_FIXED
        }
        Route::Function(n) => {
            // SAFETY: the caller's guarantee.
            unsafe { store(number, n) };
            ROUTE_FUNCTION
        }
        Route::Keysym => ROUTE_KEYSYM,
    }
}
