//! C ABI for `libobs/obs-nix-x11-input.c.h`, declared in
//! `libobs/obs-nix-x11-input.h`.
//!
//! The X11 hotkey platform (`obs-nix-x11.c`) calls these inside libobs on
//! Linux and FreeBSD. The C file has no `EXPORT`, so the symbols stay hidden
//! (`rust-hidden.txt`). Elsewhere the shims are built but unused, and hidden
//! all the same.
//!
//! The arrays are the platform's `pressed`, `update` and `button_pressed`,
//! `XINPUT_MOUSE_LEN` (`MOUSE_LEN`) `bool`s each, which the C sets only to
//! `false` and `true`. The shims read them in place and write only
//! `button_pressed`, in `obs_nix_x11_input_commit`. `obs_key_t` is a C enum,
//! passed as `int`.
//!
//! Intentional difference from C, only where C has undefined behavior: a
//! null array reads as all released, and a commit into a null array does
//! nothing (C dereferences it).

use std::ffi::c_int;

use crate::nix_x11_input::{self as core, MOUSE_LEN};

/// The `MOUSE_LEN` flags at `ptr`, or `None` for a null pointer.
///
/// # Safety
///
/// A non-null `ptr` points to `MOUSE_LEN` readable `bool`s that stay
/// unchanged for `'a`.
unsafe fn flags<'a>(ptr: *const bool) -> Option<&'a [bool; MOUSE_LEN]> {
    // SAFETY: the caller's guarantee.
    unsafe { ptr.cast::<[bool; MOUSE_LEN]>().as_ref() }
}

#[unsafe(no_mangle)]
pub extern "C" fn obs_nix_x11_input_keysym(key: c_int) -> c_int {
    core::keysym(key)
}

/// # Safety
///
/// `pressed` and `held` are null or point to `XINPUT_MOUSE_LEN` `bool`s.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_nix_x11_input_mouse_pressed(
    key: c_int,
    pressed: *const bool,
    held: *const bool,
) -> bool {
    const RELEASED: [bool; MOUSE_LEN] = [false; MOUSE_LEN];
    // SAFETY: the caller's guarantee.
    let (pressed, held) = unsafe { (flags(pressed), flags(held)) };
    core::mouse_pressed(key, pressed.unwrap_or(&RELEASED), held.unwrap_or(&RELEASED))
}

/// # Safety
///
/// `update` and `pressed` are null or point to `XINPUT_MOUSE_LEN` `bool`s;
/// `held` is null or points to `XINPUT_MOUSE_LEN` writable `bool`s that
/// overlap neither.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_nix_x11_input_commit(
    update: *const bool,
    pressed: *const bool,
    held: *mut bool,
) {
    // SAFETY: the caller's guarantees.
    let (update, pressed, held) = unsafe {
        (
            flags(update),
            flags(pressed),
            held.cast::<[bool; MOUSE_LEN]>().as_mut(),
        )
    };
    if let (Some(update), Some(pressed), Some(held)) = (update, pressed, held) {
        core::commit(update, pressed, held);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn obs_nix_x11_input_mask_pressed(key: c_int, buttons: u16) -> bool {
    core::mask_pressed(key, buttons)
}
