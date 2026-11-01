//! Safe core of `libobs/obs-nix-wayland-seats.c.h`: the seat decisions of
//! the Wayland hotkey platform (`obs-nix-wayland.c`, Linux and FreeBSD).
//!
//! The Wayland hotkey platform (still C) asks this code:
//! - [`bind_version`], for each `wl_seat` the compositor announces
//!   (`platform_registry_handler()`): which version to bind, or none for a
//!   compositor too old for hotkeys;
//! - [`pick`], whenever it has no keyboard (`take_keyboard()`): the seat to
//!   take the keyboard from;
//! - [`lacks_keyboard`], when a seat reports its capabilities
//!   (`platform_seat_capabilities()`): whether the seat has no keyboard,
//!   after which the C releases the keyboard if it came from that seat;
//! - [`find_name`], when a global is removed
//!   (`platform_registry_remover()`): which seat it was;
//! - [`uses_release`], for each seat it lets go (`release_seat()`): whether
//!   `wl_seat_release` exists at the seat's version, or the proxy is only
//!   destroyed.
//!
//! It reads the seats' capabilities and names through iterators, lazily and
//! in order, so a search reads up to the seat it finds and no further, as
//! the C loops do. The C keeps every Wayland call, the listeners, the log
//! line, the seat array and the order of each handler's steps.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::as_conversions,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable
)]

/// `WL_SEAT_CAPABILITY_KEYBOARD`: the seat has a keyboard.
pub const CAPABILITY_KEYBOARD: u32 = 2;
/// `WL_SEAT_RELEASE_SINCE_VERSION`: the first `wl_seat` version with
/// `wl_seat_release`.
pub const RELEASE_SINCE_VERSION: u32 = 5;
/// The oldest `wl_seat` version hotkeys work with.
pub const MIN_VERSION: u32 = 4;
/// The newest `wl_seat` version bound: the current one when the C was
/// written.
pub const MAX_VERSION: u32 = 7;

/// The version to bind a `wl_seat` offered at `offered`: `None` below
/// [`MIN_VERSION`] (the compositor is too old), else `offered`, at most
/// [`MAX_VERSION`].
pub fn bind_version(offered: u32) -> Option<u32> {
    (offered >= MIN_VERSION).then(|| offered.min(MAX_VERSION))
}

/// The index of the first seat whose capabilities have
/// [`CAPABILITY_KEYBOARD`], `None` for none: a compositor may announce
/// several seats, and asking one without a keyboard for it is a fatal
/// protocol error.
pub fn pick(capabilities: impl IntoIterator<Item = u32>) -> Option<usize> {
    capabilities
        .into_iter()
        .position(|caps| caps & CAPABILITY_KEYBOARD != 0)
}

/// Whether `capabilities` lack [`CAPABILITY_KEYBOARD`]: a seat reporting
/// them has no keyboard. The platform releases its keyboard when this holds
/// and only then compares the seat with the keyboard's.
pub fn lacks_keyboard(capabilities: u32) -> bool {
    capabilities & CAPABILITY_KEYBOARD == 0
}

/// The index of the first seat with registry name `name`, `None` for none.
pub fn find_name(names: impl IntoIterator<Item = u32>, name: u32) -> Option<usize> {
    names.into_iter().position(|n| n == name)
}

/// Whether a `wl_seat` bound at `version` is released (`wl_seat_release`,
/// from [`RELEASE_SINCE_VERSION`] on) rather than only destroyed.
pub fn uses_release(version: u32) -> bool {
    version >= RELEASE_SINCE_VERSION
}
