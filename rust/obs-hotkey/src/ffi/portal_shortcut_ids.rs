//! C ABI for `libobs/obs-nix-portal-shortcut-ids.c.h`, declared in
//! `libobs/obs-nix-portal-shortcuts.h`.
//!
//! The GlobalShortcuts portal client (`obs-nix-portal-shortcuts.c`) calls
//! these inside libobs on Linux and FreeBSD with `ENABLE_WAYLAND` and GIO.
//! The C file has no `EXPORT`, so the symbols stay hidden
//! (`rust-hidden.txt`). Elsewhere the shims are built but unused, and
//! hidden all the same. They call nothing from GIO, so they need no
//! feature. `bindable` is a `static inline` in the header and stays C.
//!
//! Ids and triggers are the caller's `struct dstr`, written with the same
//! `dstr` calls in the same order as the C (`dstr_free`, then one
//! `dstr_cat` per piece, `dstr_cat_dstr` for the key name), so the buffer,
//! length and capacity come out the same. `obs_key_t` is a C enum, passed
//! as `int`.
//!
//! Intentional differences from C, where C has undefined behavior: a null
//! `id` or `trigger` dstr, or a null id string or `combo` out-pointer,
//! returns false (C dereferences them, except on its own early returns); a
//! null `trigger` does not call `key_name`.

use std::ffi::{CStr, c_char, c_int, c_void};

use obs_util::ffi::darray::bfree;
use obs_util::ffi::dstr::{dstr, dstr_cat_dstr, dstr_ncat};

use super::nix_hotkey_state::obs_key_combination;
use crate::nix_hotkey_state::KeyCombination;
use crate::portal_shortcut_ids::{self as core, Text};

/// `obs_nix_portal_key_name_func`: the XKB keysym name of `key` into
/// `name`; false if there is none.
#[allow(non_camel_case_types)]
pub type obs_nix_portal_key_name_func =
    Option<unsafe extern "C" fn(key: c_int, name: *mut dstr) -> bool>;

/// A `struct dstr` as a [`Text`], written through the libobs `dstr`
/// functions.
#[repr(transparent)]
#[derive(Debug)]
pub struct DstrText(dstr);

impl Default for DstrText {
    /// `struct dstr name = {0}`.
    fn default() -> Self {
        Self(dstr {
            array: std::ptr::null_mut(),
            len: 0,
            capacity: 0,
        })
    }
}

impl DstrText {
    /// The caller's dstr.
    ///
    /// # Safety
    ///
    /// `d` is a valid, live `struct dstr`, not used through any other path
    /// for `'a`.
    unsafe fn of<'a>(d: *mut dstr) -> &'a mut Self {
        // SAFETY: `DstrText` is a transparent `dstr`; the caller vouches for
        // the pointer.
        unsafe { &mut *d.cast::<Self>() }
    }
}

impl Text for DstrText {
    /// `dstr_free` (header inline).
    fn free(&mut self) {
        // SAFETY: a dstr's array is `bmalloc`ed or null.
        unsafe { bfree(self.0.array.cast::<c_void>()) };
        self.0.array = std::ptr::null_mut();
        self.0.len = 0;
        self.0.capacity = 0;
    }

    /// `dstr_cat` (header inline): nothing for an empty string, else
    /// `dstr_ncat` with its length.
    fn cat(&mut self, s: &CStr) {
        if s.is_empty() {
            return;
        }
        // SAFETY: a valid dstr; `s` holds `count_bytes()` bytes.
        unsafe { dstr_ncat(&raw mut self.0, s.as_ptr(), s.count_bytes()) };
    }

    fn cat_text(&mut self, other: &Self) {
        // SAFETY: two valid dstrs.
        unsafe { dstr_cat_dstr(&raw mut self.0, &raw const other.0) };
    }

    /// `dstr_is_empty` (header inline).
    fn is_empty(&self) -> bool {
        // SAFETY: a non-null array of a dstr with `len > 0` holds a byte.
        self.0.array.is_null() || self.0.len == 0 || unsafe { *self.0.array } == 0
    }
}

fn combination(c: obs_key_combination) -> KeyCombination {
    KeyCombination {
        modifiers: c.modifiers,
        key: c.key,
    }
}

/// Same contract as C `obs_nix_portal_shortcut_id`.
///
/// # Safety
///
/// `id` is null or a valid `struct dstr`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_nix_portal_shortcut_id(
    combo: obs_key_combination,
    id: *mut dstr,
) -> bool {
    if id.is_null() {
        return false;
    }
    // SAFETY: a valid dstr, the caller's for this call.
    core::shortcut_id(combination(combo), unsafe { DstrText::of(id) })
}

/// Same contract as C `obs_nix_portal_shortcut_combo`.
///
/// # Safety
///
/// `id` is null or a NUL-terminated string; `combo` is null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_nix_portal_shortcut_combo(
    id: *const c_char,
    combo: *mut obs_key_combination,
) -> bool {
    if id.is_null() || combo.is_null() {
        return false;
    }
    // SAFETY: a NUL-terminated string, per the contract.
    let Some(result) = core::shortcut_combo(unsafe { CStr::from_ptr(id) }) else {
        return false;
    };
    // SAFETY: a writable `obs_key_combination_t`.
    unsafe {
        combo.write(obs_key_combination {
            modifiers: result.modifiers,
            key: result.key,
        });
    }
    true
}

/// Same contract as C `obs_nix_portal_shortcut_trigger`.
///
/// # Safety
///
/// `trigger` is null or a valid `struct dstr`; `key_name` is null or a
/// function that takes a key and a valid `struct dstr`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_nix_portal_shortcut_trigger(
    combo: obs_key_combination,
    key_name: obs_nix_portal_key_name_func,
    trigger: *mut dstr,
) -> bool {
    if trigger.is_null() {
        return false;
    }
    let key_name = key_name.map(|f| {
        move |key: i32, name: &mut DstrText| {
            // SAFETY: the caller's callback, given a valid, zeroed dstr.
            unsafe { f(key, &raw mut name.0) }
        }
    });
    // SAFETY: a valid dstr, the caller's for this call.
    core::shortcut_trigger(combination(combo), key_name, unsafe {
        DstrText::of(trigger)
    })
}
