//! `oracle_*` build of `libobs/obs-nix-wayland-key-names.c.h`
//! (oracle/nix_wayland_key_names.c).
//!
//! The function returns its C enum as `int`.

use core::ffi::{c_char, c_int};

/// Index of each value in `oracle_wayland_key_names_value`, in the C's
/// order.
pub const VALUE_NAMES: [&str; 7] = [
    "OBS_KEY_LAST_VALUE",
    "OBS_NIX_WAYLAND_KEY_NAMES_ROUTE_NONE",
    "OBS_NIX_WAYLAND_KEY_NAMES_ROUTE_MOUSE",
    "OBS_NIX_WAYLAND_KEY_NAMES_ROUTE_NUMPAD",
    "OBS_NIX_WAYLAND_KEY_NAMES_ROUTE_FIXED",
    "OBS_NIX_WAYLAND_KEY_NAMES_ROUTE_FUNCTION",
    "OBS_NIX_WAYLAND_KEY_NAMES_ROUTE_KEYSYM",
];

unsafe extern "C" {
    pub fn oracle_obs_nix_wayland_key_names_route(
        key: c_int,
        number: *mut c_int,
        name: *mut *const c_char,
    ) -> c_int;
    /// The value named `VALUE_NAMES[which]`; -1 out of range.
    pub fn oracle_wayland_key_names_value(which: usize) -> i64;
}
