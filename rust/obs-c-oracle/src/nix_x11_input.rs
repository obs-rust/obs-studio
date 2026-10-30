//! `oracle_*` build of `libobs/obs-nix-x11-input.c.h`
//! (oracle/nix_x11_input.c).
//!
//! The per-button arrays are `XINPUT_MOUSE_LEN` `bool`s each (the value
//! getter below says how many the C sees).

use core::ffi::c_int;

/// Index of each value in `oracle_x11_input_value`, in the C's order.
pub const VALUE_NAMES: [&str; 9] = [
    "OBS_KEY_LAST_VALUE",
    "OBS_KEY_NONE",
    "OBS_KEY_MOUSE1",
    "OBS_KEY_MOUSE29",
    "XINPUT_MOUSE_LEN",
    "OBS_NIX_X11_INPUT_BUTTON_MASK_1",
    "OBS_NIX_X11_INPUT_BUTTON_MASK_2",
    "OBS_NIX_X11_INPUT_BUTTON_MASK_3",
    "sizeof(bool)",
];

unsafe extern "C" {
    pub fn oracle_obs_nix_x11_input_keysym(key: c_int) -> c_int;
    pub fn oracle_obs_nix_x11_input_mouse_pressed(
        key: c_int,
        pressed: *const bool,
        held: *const bool,
    ) -> bool;
    pub fn oracle_obs_nix_x11_input_commit(
        update: *const bool,
        pressed: *const bool,
        held: *mut bool,
    );
    pub fn oracle_obs_nix_x11_input_mask_pressed(key: c_int, buttons: u16) -> bool;
    /// The value named `VALUE_NAMES[which]`; -1 out of range.
    pub fn oracle_x11_input_value(which: usize) -> i64;
}
