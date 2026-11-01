//! C ABI for `libobs/obs-nix-wayland-seats.c.h`, declared in
//! `libobs/obs-nix-wayland-seats.h`.
//!
//! The Wayland hotkey platform (`obs-nix-wayland.c`) calls these inside
//! libobs on Linux and FreeBSD with `ENABLE_WAYLAND`. The C file has no
//! `EXPORT`, so the symbols stay hidden (`rust-hidden.d`). Elsewhere the
//! shims are built but unused, and hidden all the same.
//!
//! The seat array is the platform's darray: `num` seats at `seats`, null
//! when it never grew. Only the seats' names and capabilities are read,
//! never the `wl_seat` pointer. `SIZE_MAX` stands for no seat.
//!
//! Intentional difference from C: where C has undefined behavior, a null
//! array reads as empty.

use std::ffi::c_void;

use crate::nix_wayland_seats as core;

/// `struct obs_nix_wayland_seat`: a seat the compositor announced.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct obs_nix_wayland_seat {
    /// The bound `struct wl_seat *`, opaque here.
    pub seat: *mut c_void,
    /// Its registry name.
    pub name: u32,
    /// Its last reported capabilities.
    pub capabilities: u32,
}

const _: () =
    assert!(size_of::<obs_nix_wayland_seat>() == size_of::<*mut c_void>() + 2 * size_of::<u32>());

/// `num` seats at `seats`, or none for a null pointer.
///
/// # Safety
///
/// A non-null `seats` points to `num` initialized seats that stay unchanged
/// for `'a`.
unsafe fn items<'a>(seats: *const obs_nix_wayland_seat, num: usize) -> &'a [obs_nix_wayland_seat] {
    if seats.is_null() {
        return &[];
    }
    // SAFETY: the caller's guarantee.
    unsafe { std::slice::from_raw_parts(seats, num) }
}

#[unsafe(no_mangle)]
pub extern "C" fn obs_nix_wayland_seats_bind_version(offered: u32) -> u32 {
    core::bind_version(offered).unwrap_or(0)
}

/// # Safety
///
/// `seats` is null or points to `num` seats, not written during the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_nix_wayland_seats_pick(
    seats: *const obs_nix_wayland_seat,
    num: usize,
) -> usize {
    // SAFETY: the caller's guarantee.
    let seats = unsafe { items(seats, num) };
    core::pick(seats.iter().map(|s| s.capabilities)).unwrap_or(usize::MAX)
}

#[unsafe(no_mangle)]
pub extern "C" fn obs_nix_wayland_seats_lacks_keyboard(capabilities: u32) -> bool {
    core::lacks_keyboard(capabilities)
}

/// # Safety
///
/// `seats` is null or points to `num` seats, not written during the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_nix_wayland_seats_find_name(
    seats: *const obs_nix_wayland_seat,
    num: usize,
    name: u32,
) -> usize {
    // SAFETY: the caller's guarantee.
    let seats = unsafe { items(seats, num) };
    core::find_name(seats.iter().map(|s| s.name), name).unwrap_or(usize::MAX)
}

#[unsafe(no_mangle)]
pub extern "C" fn obs_nix_wayland_seats_uses_release(version: u32) -> bool {
    core::uses_release(version)
}
