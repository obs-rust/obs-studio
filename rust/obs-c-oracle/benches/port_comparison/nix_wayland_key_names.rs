//! obs-hotkey: the Wayland key names
//! (`libobs/obs-nix-wayland-key-names.c.h`). Settings > Hotkeys and the
//! GlobalShortcuts portal descriptions name every key through
//! `obs_key_to_str`. Each side routes every key of the table once, folded
//! into a checksum both sides must match. Nothing is allocated with
//! `bmalloc`.

use super::pair;

use criterion::Criterion;
use obs_c_oracle::nix_wayland_key_names::oracle_obs_nix_wayland_key_names_route;
use obs_hotkey::ffi::nix_wayland_key_names::obs_nix_wayland_key_names_route;
use obs_hotkey::name_map::OBS_KEY_LAST_VALUE;
use std::ffi::{c_char, c_int};
use std::hint::black_box;
use std::ptr;

type Route = unsafe extern "C" fn(c_int, *mut c_int, *mut *const c_char) -> c_int;

fn run(route: Route) -> u32 {
    let mut acc = 0_u32;
    for key in 0..c_int::try_from(OBS_KEY_LAST_VALUE).unwrap() {
        let mut number: c_int = 0;
        let mut name: *const c_char = ptr::null();
        // SAFETY: both out-parameters are writable locals.
        let r = unsafe { route(black_box(key), &raw mut number, &raw mut name) };
        acc = acc
            .wrapping_mul(31)
            .wrapping_add(r as u32)
            .wrapping_add(number as u32)
            .wrapping_add(u32::from(!name.is_null()));
    }
    acc
}

pub fn benchmarks(criterion: &mut Criterion) {
    pair(
        criterion,
        "nix_wayland_key_names/route_every_key",
        || run(obs_nix_wayland_key_names_route),
        || run(oracle_obs_nix_wayland_key_names_route),
    );
}
