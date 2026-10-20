//! `oracle_*` build of `libobs/obs-nix-portal-shortcut-ids.c.h`
//! (oracle/portal_shortcut_ids.c).
//!
//! Ids and triggers are `struct dstr`s, passed as untyped pointers: callers
//! use `obs_util::ffi::dstr::dstr`, whose layout the dstr oracle checks.

use core::ffi::{c_char, c_int, c_void};

pub use crate::nix_hotkey_state::OracleKeyCombination;

/// `obs_nix_portal_key_name_func`, with the dstr untyped.
pub type OracleKeyNameFn = Option<unsafe extern "C" fn(key: c_int, name: *mut c_void) -> bool>;

/// Index of each value in `oracle_portal_value`, in the C's order.
pub const VALUE_NAMES: [&str; 8] = [
    "OBS_KEY_NONE",
    "OBS_KEY_LAST_VALUE",
    "OBS_KEY_MOUSE1",
    "OBS_KEY_MOUSE29",
    "INTERACT_CONTROL_KEY",
    "INTERACT_ALT_KEY",
    "INTERACT_SHIFT_KEY",
    "INTERACT_COMMAND_KEY",
];

unsafe extern "C" {
    pub fn oracle_obs_nix_portal_shortcut_id(combo: OracleKeyCombination, id: *mut c_void) -> bool;
    pub fn oracle_obs_nix_portal_shortcut_combo(
        id: *const c_char,
        combo: *mut OracleKeyCombination,
    ) -> bool;
    pub fn oracle_obs_nix_portal_shortcut_trigger(
        combo: OracleKeyCombination,
        key_name: OracleKeyNameFn,
        trigger: *mut c_void,
    ) -> bool;
    /// `bindable` from `obs-nix-portal-shortcuts.h`.
    pub fn oracle_portal_bindable(combo: OracleKeyCombination) -> bool;
    /// The value named `VALUE_NAMES[which]`; -1 out of range.
    pub fn oracle_portal_value(which: usize) -> i64;
    /// Entry `which` of `modifier_names`; false past its end.
    pub fn oracle_portal_modifier(
        which: usize,
        modifier: *mut u32,
        id: *mut *const c_char,
        trigger: *mut *const c_char,
    ) -> bool;
}
