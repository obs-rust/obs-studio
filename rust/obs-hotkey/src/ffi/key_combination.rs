//! C ABI for `libobs/obs-key-combination-host-posix.c.h`:
//! `obs_key_combination_to_str()`, declared `EXPORT` in `obs-hotkey.h`.
//!
//! Built on Linux and FreeBSD only: `ffi/mod.rs` leaves the module out on
//! macOS (`obs-cocoa.m` keeps its C definition), and the shim below is
//! `cfg(unix)`, so Windows (`obs-windows.c`) keeps its own too. The export
//! is marked `unix` in `libobs/cmake/rust-exports.d`.
//!
//! Each name is written by the C `obs_key_to_str()` (the X11 or Wayland
//! platform's, still C) into a fresh `struct dstr = {0}`, appended to the
//! caller's `dstr` with `dstr_cat` and `dstr_cat_dstr`, and freed with
//! `dstr_free`, as the C does, so the buffer, length and capacity come out
//! the same.
//!
//! Intentional difference from C, where C has undefined behavior: a null
//! `str` does nothing (C dereferences it once a key has a name).

#[cfg(unix)]
mod shims {
    use std::ffi::c_int;

    use obs_util::ffi::dstr::dstr;

    use crate::ffi::nix_hotkey_state::obs_key_combination;
    use crate::ffi::portal_shortcut_ids::DstrText;
    use crate::key_combination as core;
    use crate::nix_hotkey_state::KeyCombination;

    unsafe extern "C" {
        /// `obs_key_to_str` (`obs-hotkey.h`): the platform's name of `key`.
        fn obs_key_to_str(key: c_int, str: *mut dstr);
    }

    /// Same contract as C `obs_key_combination_to_str`.
    ///
    /// # Safety
    ///
    /// `str` is null or a valid `struct dstr`.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn obs_key_combination_to_str(
        combination: obs_key_combination,
        str: *mut dstr,
    ) {
        if str.is_null() {
            return;
        }
        let combination = KeyCombination {
            modifiers: combination.modifiers,
            key: combination.key,
        };
        // SAFETY: a valid dstr, the caller's for this call.
        let text = unsafe { DstrText::of(str) };
        core::combination_to_str(
            combination,
            |key, name: &mut DstrText| {
                // SAFETY: `name` is a live dstr this call owns.
                unsafe { obs_key_to_str(key, name.as_mut_ptr()) };
            },
            text,
        );
    }
}

#[cfg(unix)]
pub use shims::*;
