//! `oracle_*` build of `libobs/obs-nix-x11-hotkeys.c.h`
//! (oracle/nix_x11_hotkeys.c).
//!
//! Both functions return their C enum as `int`.

use core::ffi::{c_char, c_int};

/// Index of each value in `oracle_x11_hotkeys_value`, in the C's order.
pub const VALUE_NAMES: [&str; 5] = [
    "OBS_KEY_LAST_VALUE",
    "XINPUT_MOUSE_LEN",
    "OBS_NIX_X11_HOTKEYS_GE_GENERIC",
    "OBS_NIX_X11_HOTKEYS_RAW_BUTTON_PRESS",
    "OBS_NIX_X11_HOTKEYS_RAW_BUTTON_RELEASE",
];

unsafe extern "C" {
    pub fn oracle_obs_nix_x11_hotkeys_route(
        key: c_int,
        number: *mut c_int,
        name: *mut *const c_char,
    ) -> c_int;
    pub fn oracle_obs_nix_x11_hotkeys_raw_button(
        response_type: u8,
        event_type: *const u16,
        detail: *const u32,
        slot: *mut u32,
    ) -> c_int;
    /// The value named `VALUE_NAMES[which]`; -1 out of range.
    pub fn oracle_x11_hotkeys_value(which: usize) -> i64;
}
