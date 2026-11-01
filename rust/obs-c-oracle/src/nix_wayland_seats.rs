//! `oracle_*` build of `libobs/obs-nix-wayland-seats.c.h`
//! (oracle/nix_wayland_seats.c).

use core::ffi::c_void;

/// Index of each value in `oracle_wayland_seats_value`, in the C's order.
pub const VALUE_NAMES: [&str; 7] = [
    "OBS_NIX_WAYLAND_SEATS_CAPABILITY_KEYBOARD",
    "OBS_NIX_WAYLAND_SEATS_RELEASE_SINCE_VERSION",
    "sizeof(struct obs_nix_wayland_seat)",
    "alignof(struct obs_nix_wayland_seat)",
    "offsetof(struct obs_nix_wayland_seat, seat)",
    "offsetof(struct obs_nix_wayland_seat, name)",
    "offsetof(struct obs_nix_wayland_seat, capabilities)",
];

/// `struct obs_nix_wayland_seat`, as the oracle reads it.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Seat {
    pub seat: *mut c_void,
    pub name: u32,
    pub capabilities: u32,
}

unsafe extern "C" {
    pub fn oracle_obs_nix_wayland_seats_bind_version(offered: u32) -> u32;
    pub fn oracle_obs_nix_wayland_seats_pick(seats: *const Seat, num: usize) -> usize;
    pub fn oracle_obs_nix_wayland_seats_lacks_keyboard(capabilities: u32) -> bool;
    pub fn oracle_obs_nix_wayland_seats_find_name(
        seats: *const Seat,
        num: usize,
        name: u32,
    ) -> usize;
    pub fn oracle_obs_nix_wayland_seats_uses_release(version: u32) -> bool;
    /// The value at `which` in [`VALUE_NAMES`]' order, -1 past the end.
    pub fn oracle_wayland_seats_value(which: usize) -> i64;
}
