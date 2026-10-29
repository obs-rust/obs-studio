//! C ABI for `libobs/obs-nix-x11-keys.c.h`, declared in
//! `libobs/obs-nix-x11-keys.h`.
//!
//! The X11 hotkey platform (`obs-nix-x11.c`) calls these inside libobs on
//! Linux and FreeBSD. The C file has no `EXPORT`, so the symbols stay hidden
//! (`rust-hidden.txt`). Elsewhere the shims are built but unused, and hidden
//! all the same.
//!
//! The tables are the platform's: `base_keysyms` and `keycodes` have
//! `OBS_KEY_LAST_VALUE` entries, the keyboard map has `num_keysyms`
//! symbols (its `bmemdup` copy), and each keycode list is its darray of
//! `uint8_t`. The shims only read them, and each field the C read only
//! under a condition (passed by pointer, or for the base table through the
//! caller's getter) only under that condition. `obs_key_t` and
//! `enum obs_nix_x11_keys_row` are C enums, passed as `int`.
//!
//! Intentional differences from C:
//! - `obs_nix_x11_keys_to_virtual_key` returns 0 for a key outside
//!   `0 .. OBS_KEY_LAST_VALUE - 1`; C indexes the table with it;
//! - where C has undefined behavior, a null table, map, list or keycode
//!   pointer reads as empty or 0, and a null getter as an empty table (C
//!   dereferences or calls it).

use std::ffi::c_int;

use obs_util::ffi::darray::darray;

use crate::nix_x11_keys::{self as core, KEYMAP_LEN, OBS_KEY_LAST_VALUE, Row};

/// `OBS_NIX_X11_KEYS_ROW_NONE`.
pub const OBS_NIX_X11_KEYS_ROW_NONE: c_int = 0;
/// `OBS_NIX_X11_KEYS_ROW_SUPER_L`.
pub const OBS_NIX_X11_KEYS_ROW_SUPER_L: c_int = 1;
/// `OBS_NIX_X11_KEYS_ROW_SUPER_R`.
pub const OBS_NIX_X11_KEYS_ROW_SUPER_R: c_int = 2;
/// `OBS_NIX_X11_KEYS_ROW_KEY`.
pub const OBS_NIX_X11_KEYS_ROW_KEY: c_int = 3;

/// `struct keycode_list` in `obs-nix-x11-keys.h`: `DARRAY(uint8_t) list`.
///
/// Safe code only makes an empty one ([`Default`]); any other comes from C,
/// whose darray holds `num` initialized keycodes, so [`AsRef`] relies on
/// that.
#[repr(transparent)]
#[derive(Debug)]
#[allow(non_camel_case_types)]
pub struct keycode_list(darray);

impl Default for keycode_list {
    fn default() -> Self {
        Self(darray {
            array: std::ptr::null_mut(),
            num: 0,
            capacity: 0,
        })
    }
}

impl keycode_list {
    /// The raw darray: buffer, item count and capacity.
    pub fn raw(&self) -> &darray {
        &self.0
    }
}

impl AsRef<[u8]> for keycode_list {
    fn as_ref(&self) -> &[u8] {
        if self.0.num == 0 || self.0.array.is_null() {
            return &[];
        }
        // SAFETY: a non-empty darray from C holds `num` initialized keycodes.
        unsafe { std::slice::from_raw_parts(self.0.array.cast::<u8>(), self.0.num) }
    }
}

/// `len` items at `ptr`, or none for a null pointer or a length of 0.
///
/// # Safety
///
/// A non-null `ptr` points to `len` readable, initialized items that stay
/// unchanged for `'a`.
unsafe fn items<'a, T>(ptr: *const T, len: usize) -> &'a [T] {
    if ptr.is_null() || len == 0 {
        return &[];
    }
    // SAFETY: the caller's guarantee.
    unsafe { std::slice::from_raw_parts(ptr, len) }
}

/// `len` items at `ptr` for a C `int` length; none for 0 or less.
///
/// # Safety
///
/// As [`items`].
unsafe fn items_int<'a, T>(ptr: *const T, len: c_int) -> &'a [T] {
    // SAFETY: the caller's guarantee.
    unsafe { items(ptr, usize::try_from(len).unwrap_or(0)) }
}

/// # Safety
///
/// `base_keysyms` points to `OBS_KEY_LAST_VALUE` keysyms.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_nix_x11_keys_from_base_keysym(
    base_keysyms: *const u32,
    code: u32,
) -> c_int {
    // SAFETY: the caller's guarantee.
    let table = unsafe { items(base_keysyms, OBS_KEY_LAST_VALUE) };
    core::key_from_base(table, code)
}

/// # Safety
///
/// For `syms_per_code` above 0, `row` points to that many keysyms;
/// `base_keysyms` points to `OBS_KEY_LAST_VALUE` keysyms; `key` is
/// writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_nix_x11_keys_row(
    row: *const u32,
    syms_per_code: c_int,
    base_keysyms: *const u32,
    key: *mut c_int,
) -> c_int {
    // SAFETY: the caller's guarantees.
    let (row, table) = unsafe {
        (
            items_int(row, syms_per_code),
            items(base_keysyms, OBS_KEY_LAST_VALUE),
        )
    };
    match core::row_key(row, table) {
        Row::None => OBS_NIX_X11_KEYS_ROW_NONE,
        Row::SuperL => OBS_NIX_X11_KEYS_ROW_SUPER_L,
        Row::SuperR => OBS_NIX_X11_KEYS_ROW_SUPER_R,
        Row::Key(found) => {
            // Written only for a key, as the C does.
            // SAFETY: the caller guarantees `key` is writable.
            unsafe { key.write(found) };
            OBS_NIX_X11_KEYS_ROW_KEY
        }
    }
}

/// `*ptr`, or 0 for a null pointer.
///
/// # Safety
///
/// A non-null `ptr` is readable.
unsafe fn read_or_zero<T: Copy + Default>(ptr: *const T) -> T {
    // SAFETY: the caller's guarantee.
    unsafe { ptr.as_ref() }.copied().unwrap_or_default()
}

/// # Safety
///
/// `keys` is null or points to a QueryKeymap reply's 32-byte bitmap;
/// `codes`, `super_l_code` and `super_r_code` are null or readable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_nix_x11_keys_pressed(
    keys: *const u8,
    key: c_int,
    codes: *const keycode_list,
    super_l_code: *const u8,
    super_r_code: *const u8,
) -> bool {
    // SAFETY: the caller's guarantee.
    let keys = unsafe { keys.cast::<[u8; KEYMAP_LEN]>().as_ref() };
    core::any_pressed(
        keys,
        key,
        // SAFETY: the caller's guarantee; read only when the core asks.
        || unsafe { codes.as_ref() }.map_or(&[][..], AsRef::as_ref),
        // SAFETY: as above.
        || unsafe { read_or_zero(super_l_code) },
        // SAFETY: as above.
        || unsafe { read_or_zero(super_r_code) },
    )
}

/// # Safety
///
/// `keycodes` points to `OBS_KEY_LAST_VALUE` keycode lists.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_nix_x11_keys_from_keycode(
    keycodes: *const keycode_list,
    code: u8,
) -> c_int {
    // SAFETY: the caller's guarantee.
    let lists = unsafe { items(keycodes, OBS_KEY_LAST_VALUE) };
    core::key_from_keycode(lists, code)
}

/// # Safety
///
/// `keysyms` is null or points to `*num_keysyms` keysyms; `num_keysyms` and
/// `min_keycode` are null or readable; `keycodes` is null or points to
/// `OBS_KEY_LAST_VALUE` keycode lists.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_nix_x11_keys_from_virtual_key(
    sym: c_int,
    keysyms: *const u32,
    syms_per_code: c_int,
    num_keysyms: *const c_int,
    min_keycode: *const u8,
    keycodes: *const keycode_list,
) -> c_int {
    core::from_virtual(
        sym,
        syms_per_code,
        // SAFETY: the caller's guarantees; read only when the core asks.
        || unsafe { items_int(keysyms, read_or_zero(num_keysyms)) },
        // SAFETY: as above.
        || unsafe { read_or_zero(min_keycode) },
        // SAFETY: as above.
        || unsafe { items(keycodes, OBS_KEY_LAST_VALUE) },
    )
}

/// `obs_nix_x11_keys_table_fn`: the base keysym table's getter.
pub type TableFn = Option<unsafe extern "C" fn() -> *const u32>;

/// # Safety
///
/// `base_keysyms` is null or returns a pointer to `OBS_KEY_LAST_VALUE`
/// keysyms.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_nix_x11_keys_to_virtual_key(
    key: c_int,
    base_keysyms: TableFn,
) -> c_int {
    core::to_virtual(key, || match base_keysyms {
        // SAFETY: the caller's guarantee; called only when the core asks.
        Some(table) => unsafe { items(table(), OBS_KEY_LAST_VALUE) },
        None => &[],
    })
}
