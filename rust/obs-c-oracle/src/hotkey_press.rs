//! `oracle_*` build of `libobs/obs-hotkey-press.c.h`
//! (oracle/hotkey_press.c).
//!
//! A binding is an untyped pointer: the oracle reaches it only through
//! `obs_hotkey_binding_fields()`, which the caller (the test binary)
//! defines, with `obs_hotkey_key_is_pressed()`, `obs_hotkey_binding_press()`
//! and `obs_hotkey_binding_release()`.

use core::ffi::c_void;

pub use crate::nix_hotkey_state::OracleKeyCombination;

/// Index of each value in `oracle_hotkey_press_value`, in the C's order.
pub const VALUE_NAMES: [&str; 9] = [
    "OBS_KEY_NONE",
    "OBS_KEY_SHIFT",
    "OBS_KEY_CONTROL",
    "OBS_KEY_ALT",
    "OBS_KEY_META",
    "INTERACT_SHIFT_KEY",
    "INTERACT_CONTROL_KEY",
    "INTERACT_ALT_KEY",
    "INTERACT_COMMAND_KEY",
];

/// Index of each entry in `oracle_hotkey_press_layout`, in the C's order.
pub const LAYOUT_NAMES: [&str; 9] = [
    "sizeof(struct obs_hotkey_binding_fields)",
    "alignof(struct obs_hotkey_binding_fields)",
    "offsetof(struct obs_hotkey_binding_fields, key)",
    "offsetof(struct obs_hotkey_binding_fields, pressed)",
    "offsetof(struct obs_hotkey_binding_fields, modifiers_match)",
    "sizeof(obs_key_combination_t)",
    "alignof(obs_key_combination_t)",
    "offsetof(obs_key_combination_t, modifiers)",
    "offsetof(obs_key_combination_t, key)",
];

unsafe extern "C" {
    pub fn oracle_obs_hotkey_binding_query(
        binding: *mut c_void,
        modifiers: u32,
        no_press: bool,
        strict_modifiers: bool,
    );
    pub fn oracle_obs_hotkey_binding_inject(
        binding: *mut c_void,
        hotkey: OracleKeyCombination,
        pressed: bool,
        strict_modifiers: bool,
    );
    pub fn oracle_obs_hotkeys_query_modifiers() -> u32;
    /// The value named `VALUE_NAMES[which]`; -1 out of range.
    pub fn oracle_hotkey_press_value(which: usize) -> i64;
    /// The entry named `LAYOUT_NAMES[which]`; 0 out of range.
    pub fn oracle_hotkey_press_layout(which: usize) -> usize;
}
