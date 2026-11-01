//! obs-hotkey: the Wayland seat decisions (`libobs/obs-nix-wayland-seats.c.h`).
//! The Wayland hotkey platform asks them when the compositor announces a
//! seat, when a seat's capabilities change and when a global is removed. On
//! eight seats (keyboards on the last two), each side answers 64 rounds of
//! a bind version, a capability change (the keyboard bit test and the
//! keyboard pick) and a removal (the name lookup and the release rule),
//! folded into a checksum both sides must match. The seats are an array
//! the bench owns; nothing is allocated with `bmalloc`.

use super::pair;

use criterion::Criterion;
use obs_c_oracle::nix_wayland_seats::{
    Seat as OracleSeat, oracle_obs_nix_wayland_seats_bind_version,
    oracle_obs_nix_wayland_seats_find_name, oracle_obs_nix_wayland_seats_pick,
    oracle_obs_nix_wayland_seats_lacks_keyboard, oracle_obs_nix_wayland_seats_uses_release,
};
use obs_hotkey::ffi::nix_wayland_seats::{
    obs_nix_wayland_seat, obs_nix_wayland_seats_bind_version, obs_nix_wayland_seats_find_name,
    obs_nix_wayland_seats_pick, obs_nix_wayland_seats_lacks_keyboard,
    obs_nix_wayland_seats_uses_release,
};
use std::hint::black_box;

const SEATS: usize = 8;
const ROUNDS: usize = 64;

fn seats() -> Vec<obs_nix_wayland_seat> {
    (0..SEATS)
        .map(|i| obs_nix_wayland_seat {
            seat: std::ptr::null_mut(),
            name: 20 + u32::try_from(i).unwrap(),
            capabilities: if i + 2 >= SEATS { 3 } else { 1 },
        })
        .collect()
}

/// Round `i`'s offered version, capabilities and removed name.
fn round(i: usize) -> (u32, u32, u32) {
    let i = u32::try_from(i).unwrap();
    (i % 10, i % 8, 18 + i % 12)
}

fn rust_rounds(s: &[obs_nix_wayland_seat]) -> usize {
    let mut acc = 0_usize;
    for i in 0..ROUNDS {
        let (version, caps, name) = black_box(round(i));
        let bind = obs_nix_wayland_seats_bind_version(version);
        let release = obs_nix_wayland_seats_lacks_keyboard(caps) && i % 3 == 0;
        // SAFETY: SEATS seats, alive and unchanged for the calls.
        let (pick, found) = unsafe {
            (
                obs_nix_wayland_seats_pick(s.as_ptr(), SEATS),
                obs_nix_wayland_seats_find_name(s.as_ptr(), SEATS, name),
            )
        };
        let uses = obs_nix_wayland_seats_uses_release(bind);
        acc = acc.wrapping_mul(31).wrapping_add(bind as usize);
        acc = acc.wrapping_mul(31).wrapping_add(usize::from(release) + 2 * usize::from(uses));
        acc = acc.wrapping_mul(31).wrapping_add(pick).wrapping_add(found);
    }
    acc
}

fn c_rounds(s: &[obs_nix_wayland_seat]) -> usize {
    let p: *const OracleSeat = s.as_ptr().cast();
    let mut acc = 0_usize;
    for i in 0..ROUNDS {
        let (version, caps, name) = black_box(round(i));
        // SAFETY: as rust_rounds, with the oracle's identical layout; the
        // oracle keeps no state.
        let (bind, release, pick, found) = unsafe {
            (
                oracle_obs_nix_wayland_seats_bind_version(version),
                oracle_obs_nix_wayland_seats_lacks_keyboard(caps) && i % 3 == 0,
                oracle_obs_nix_wayland_seats_pick(p, SEATS),
                oracle_obs_nix_wayland_seats_find_name(p, SEATS, name),
            )
        };
        // SAFETY: a plain value.
        let uses = unsafe { oracle_obs_nix_wayland_seats_uses_release(bind) };
        acc = acc.wrapping_mul(31).wrapping_add(bind as usize);
        acc = acc.wrapping_mul(31).wrapping_add(usize::from(release) + 2 * usize::from(uses));
        acc = acc.wrapping_mul(31).wrapping_add(pick).wrapping_add(found);
    }
    acc
}

pub fn benchmarks(criterion: &mut Criterion) {
    let s = seats();
    pair(
        criterion,
        "nix_wayland_seats/rounds/64",
        || rust_rounds(&s),
        || c_rounds(&s),
    );
}
