//! obs-hotkey: the X11 key names and raw button decode
//! (`libobs/obs-nix-x11-hotkeys.c.h`). Settings > Hotkeys names every key
//! through `obs_key_to_str`, and the hotkey thread decodes each raw event it
//! drains every 25 ms. Each side routes every key once and decodes 1024
//! events (generic and other response types, raw presses, releases and
//! other event types, buttons 0 to 39), folded into a checksum both sides
//! must match outside the three intentional differences, which the events
//! avoid. Nothing is allocated with `bmalloc`.

use super::pair;

use criterion::Criterion;
use obs_c_oracle::nix_x11_hotkeys::{
    oracle_obs_nix_x11_hotkeys_raw_button, oracle_obs_nix_x11_hotkeys_route,
};
use obs_hotkey::ffi::nix_x11_hotkeys::{obs_nix_x11_hotkeys_raw_button, obs_nix_x11_hotkeys_route};
use obs_hotkey::name_map::OBS_KEY_LAST_VALUE;
use std::ffi::{c_char, c_int};
use std::hint::black_box;
use std::ptr;

const EVENTS: usize = 1024;

/// Event `i`: a response type both sides agree on (35, or one neither takes
/// for a generic event), an event type around the raw button ones and a
/// button from 1 to 40.
fn event(i: usize) -> (u8, u16, u32) {
    let response_type = if i.is_multiple_of(4) { 2 } else { 35 };
    let event_type = 14 + (i % 4) as u16;
    let detail = 1 + (i * 7 % 40) as u32;
    (response_type, event_type, detail)
}

type Route = unsafe extern "C" fn(c_int, *mut c_int, *mut *const c_char) -> c_int;
type RawButton = unsafe extern "C" fn(u8, *const u16, *const u32, *mut u32) -> c_int;

fn run(route: Route, raw_button: RawButton) -> u32 {
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
    for i in 0..EVENTS {
        let (response_type, event_type, detail) = black_box(event(i));
        let mut slot = 0_u32;
        // SAFETY: each pointer points to a live local.
        let b = unsafe { raw_button(response_type, &event_type, &detail, &raw mut slot) };
        acc = acc.wrapping_mul(31).wrapping_add(b as u32).wrapping_add(slot);
    }
    acc
}

pub fn benchmarks(criterion: &mut Criterion) {
    pair(
        criterion,
        "nix_x11_hotkeys/route_and_decode/1024",
        || run(obs_nix_x11_hotkeys_route, obs_nix_x11_hotkeys_raw_button),
        || {
            run(
                oracle_obs_nix_x11_hotkeys_route,
                oracle_obs_nix_x11_hotkeys_raw_button,
            )
        },
    );
}
