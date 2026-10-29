//! `oracle_*` build of `libobs/obs-nix-x11-keys.c.h` (oracle/nix_x11_keys.c).
//!
//! The keycode lists are passed as an untyped pointer: callers lay out
//! `struct keycode_list` (a `DARRAY(uint8_t)`) themselves (the layout
//! getter below says how the C sees it).

use core::ffi::{c_int, c_void};

/// Index of each value in `oracle_x11_keys_value`, in the C's order.
pub const VALUE_NAMES: [&str; 15] = [
    "OBS_KEY_LAST_VALUE",
    "OBS_KEY_NONE",
    "OBS_KEY_SHIFT",
    "OBS_KEY_CONTROL",
    "OBS_KEY_ALT",
    "OBS_KEY_META",
    "OBS_NIX_X11_KEYS_SHIFT_R",
    "OBS_NIX_X11_KEYS_CONTROL_R",
    "OBS_NIX_X11_KEYS_ALT_R",
    "OBS_NIX_X11_KEYS_SUPER_L",
    "OBS_NIX_X11_KEYS_SUPER_R",
    "OBS_NIX_X11_KEYS_ROW_NONE",
    "OBS_NIX_X11_KEYS_ROW_SUPER_L",
    "OBS_NIX_X11_KEYS_ROW_SUPER_R",
    "OBS_NIX_X11_KEYS_ROW_KEY",
];

/// Index of each entry in `oracle_x11_keys_layout`, in the C's order.
pub const LAYOUT_NAMES: [&str; 5] = [
    "sizeof(struct keycode_list)",
    "alignof(struct keycode_list)",
    "offsetof(struct keycode_list, list.array)",
    "offsetof(struct keycode_list, list.num)",
    "offsetof(struct keycode_list, list.capacity)",
];

unsafe extern "C" {
    pub fn oracle_obs_nix_x11_keys_from_base_keysym(base_keysyms: *const u32, code: u32) -> c_int;
    pub fn oracle_obs_nix_x11_keys_row(
        row: *const u32,
        syms_per_code: c_int,
        base_keysyms: *const u32,
        key: *mut c_int,
    ) -> c_int;
    pub fn oracle_obs_nix_x11_keys_pressed(
        keys: *const u8,
        key: c_int,
        codes: *const c_void,
        super_l_code: *const u8,
        super_r_code: *const u8,
    ) -> bool;
    pub fn oracle_obs_nix_x11_keys_from_keycode(keycodes: *const c_void, code: u8) -> c_int;
    pub fn oracle_obs_nix_x11_keys_from_virtual_key(
        sym: c_int,
        keysyms: *const u32,
        syms_per_code: c_int,
        num_keysyms: *const c_int,
        min_keycode: *const u8,
        keycodes: *const c_void,
    ) -> c_int;
    pub fn oracle_obs_nix_x11_keys_to_virtual_key(
        key: c_int,
        base_keysyms: Option<unsafe extern "C" fn() -> *const u32>,
    ) -> c_int;
    /// The value named `VALUE_NAMES[which]`; -1 out of range.
    pub fn oracle_x11_keys_value(which: usize) -> i64;
    /// The entry named `LAYOUT_NAMES[which]`; 0 out of range.
    pub fn oracle_x11_keys_layout(which: usize) -> usize;
}
