//! `oracle_*` build of `libobs/obs-hotkey-bindings.c.h`
//! (oracle/hotkey_bindings.c).
//!
//! obs_data objects are passed as untyped pointers: callers use the Rust
//! obs-data port, which both this oracle and the shims read and write.

use core::ffi::c_void;

pub use crate::nix_hotkey_state::OracleKeyCombination;

/// Index of each value in `oracle_hotkey_bindings_value`, in the C's order.
pub const VALUE_NAMES: [&str; 8] = [
    "OBS_KEY_NONE",
    "OBS_KEY_LAST_VALUE",
    "OBS_KEY_MOUSE1",
    "OBS_KEY_MOUSE2",
    "INTERACT_SHIFT_KEY",
    "INTERACT_CONTROL_KEY",
    "INTERACT_ALT_KEY",
    "INTERACT_COMMAND_KEY",
];

unsafe extern "C" {
    pub fn oracle_obs_hotkey_combination_from_data(
        data: *mut c_void,
        combo: *mut OracleKeyCombination,
    ) -> bool;
    pub fn oracle_obs_hotkey_combination_bindable(combo: OracleKeyCombination) -> bool;
    pub fn oracle_obs_hotkey_combination_to_data(combo: OracleKeyCombination, hotkey: *mut c_void);
    /// The value named `VALUE_NAMES[which]`; -1 out of range.
    pub fn oracle_hotkey_bindings_value(which: usize) -> i64;
}
