//! `oracle_*` build of `libobs/obs-cocoa-keys.c.h` (oracle/cocoa_keys.c).
//!
//! The table is passed as an untyped pointer: callers lay out
//! `obs_key_code_t` (`{ int code; bool is_valid; }`) themselves (the layout
//! getter below says how the C sees it).

use core::ffi::{c_int, c_void};

/// Index of each value in `oracle_cocoa_keys_value`, in the C's order.
pub const VALUE_NAMES: [&str; 13] = [
    "OBS_KEY_LAST_VALUE",
    "OBS_KEY_NONE",
    "OBS_KEY_SHIFT",
    "OBS_KEY_CONTROL",
    "OBS_KEY_ALT",
    "OBS_KEY_META",
    "OBS_KEY_MOUSE1",
    "OBS_KEY_MOUSE29",
    "OBS_COCOA_KEYS_FLAG_SHIFT",
    "OBS_COCOA_KEYS_FLAG_CONTROL",
    "OBS_COCOA_KEYS_FLAG_ALTERNATE",
    "OBS_COCOA_KEYS_FLAG_COMMAND",
    "OBS_COCOA_KEYS_INVALID",
];

/// Index of each entry in `oracle_cocoa_keys_layout`, in the C's order.
pub const LAYOUT_NAMES: [&str; 4] = [
    "sizeof(obs_key_code_t)",
    "alignof(obs_key_code_t)",
    "offsetof(obs_key_code_t, code)",
    "offsetof(obs_key_code_t, is_valid)",
];

unsafe extern "C" {
    pub fn oracle_obs_cocoa_keys_modifier(flags: u64, key: c_int) -> bool;
    pub fn oracle_obs_cocoa_keys_to_virtual_key(table: *const c_void, key: c_int) -> c_int;
    pub fn oracle_obs_cocoa_keys_from_virtual_key(table: *const c_void, code: c_int) -> c_int;
    pub fn oracle_obs_cocoa_keys_is_mouse(key: c_int) -> bool;
    /// Undefined for every mouse key (the shift is past an `int`'s width):
    /// declared for completeness, never called.
    pub fn oracle_obs_cocoa_keys_mouse_pressed(key: c_int, buttons: u64) -> bool;
    pub fn oracle_obs_cocoa_keys_in_range(key: c_int) -> bool;
    /// The value named `VALUE_NAMES[which]`; -1 out of range.
    pub fn oracle_cocoa_keys_value(which: usize) -> i64;
    /// The entry named `LAYOUT_NAMES[which]`; 0 out of range.
    pub fn oracle_cocoa_keys_layout(which: usize) -> usize;
}
