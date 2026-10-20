//! `oracle_*` build of `libobs/obs-nix-hotkey-state.c.h`
//! (oracle/nix_hotkey_state.c).
//!
//! The state is passed as an untyped pointer: callers lay out
//! `struct obs_nix_hotkey_state` themselves (the layout getters below say
//! how the C sees it).

use core::ffi::{c_int, c_void};

/// `obs_key_combination_t`, as the oracle sees it.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct OracleKeyCombination {
    pub modifiers: u32,
    pub key: c_int,
}

/// Index of each value in `oracle_hotkey_state_value`, in the C's order.
pub const VALUE_NAMES: [&str; 9] = [
    "OBS_KEY_LAST_VALUE",
    "OBS_KEY_SHIFT",
    "OBS_KEY_CONTROL",
    "OBS_KEY_ALT",
    "OBS_KEY_META",
    "INTERACT_SHIFT_KEY",
    "INTERACT_CONTROL_KEY",
    "INTERACT_ALT_KEY",
    "INTERACT_COMMAND_KEY",
];

/// Index of each entry in `oracle_hotkey_state_layout`, in the C's order.
pub const LAYOUT_NAMES: [&str; 13] = [
    "sizeof(obs_key_combination_t)",
    "alignof(obs_key_combination_t)",
    "offsetof(obs_key_combination_t, modifiers)",
    "offsetof(obs_key_combination_t, key)",
    "sizeof(struct obs_nix_hotkey_shortcut)",
    "alignof(struct obs_nix_hotkey_shortcut)",
    "offsetof(struct obs_nix_hotkey_shortcut, combo)",
    "offsetof(struct obs_nix_hotkey_shortcut, observed)",
    "offsetof(struct obs_nix_hotkey_shortcut, release_pending)",
    "sizeof(struct obs_nix_hotkey_state)",
    "alignof(struct obs_nix_hotkey_state)",
    "offsetof(struct obs_nix_hotkey_state, focused_modifiers)",
    "offsetof(struct obs_nix_hotkey_state, shortcuts)",
];

unsafe extern "C" {
    pub fn oracle_obs_nix_hotkey_state_free(state: *mut c_void);
    pub fn oracle_obs_nix_hotkey_state_set_key(state: *mut c_void, key: c_int, pressed: bool);
    pub fn oracle_obs_nix_hotkey_state_set_modifiers(state: *mut c_void, modifiers: u32);
    pub fn oracle_obs_nix_hotkey_state_clear_keys(state: *mut c_void);
    pub fn oracle_obs_nix_hotkey_state_set_shortcut(
        state: *mut c_void,
        combo: OracleKeyCombination,
        active: bool,
    );
    pub fn oracle_obs_nix_hotkey_state_flush(state: *mut c_void) -> bool;
    pub fn oracle_obs_nix_hotkey_state_is_pressed(state: *mut c_void, key: c_int) -> bool;
    /// The value named `VALUE_NAMES[which]`; -1 out of range.
    pub fn oracle_hotkey_state_value(which: usize) -> i64;
    /// The entry named `LAYOUT_NAMES[which]`; 0 out of range.
    pub fn oracle_hotkey_state_layout(which: usize) -> usize;
}
