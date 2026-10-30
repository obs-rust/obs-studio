//! obs-hotkey: the X11 keysym table and mouse button answers
//! (`libobs/obs-nix-x11-input.c.h`). Hotkey init asks the keysym of every
//! key (`fill_base_keysyms`), and the hotkey thread asks every 25 ms whether
//! each bound mouse key is down, then commits the poll. Each side fills the
//! base table once and runs 64 polls (the answer of all 29 mouse keys and
//! the commit, over arrays that change each poll), folded into a checksum
//! both sides must match. The arrays are the bench's own; nothing is
//! allocated with `bmalloc`.

use super::pair;

use criterion::Criterion;
use obs_c_oracle::nix_x11_input::{
    oracle_obs_nix_x11_input_commit, oracle_obs_nix_x11_input_keysym,
    oracle_obs_nix_x11_input_mouse_pressed,
};
use obs_hotkey::ffi::nix_x11_input::{
    obs_nix_x11_input_commit, obs_nix_x11_input_keysym, obs_nix_x11_input_mouse_pressed,
};
use obs_hotkey::name_map::{OBS_KEY_LAST_VALUE, OBS_KEY_MOUSE1, OBS_KEY_MOUSE29};
use obs_hotkey::nix_x11_input::MOUSE_LEN;
use std::ffi::c_int;
use std::hint::black_box;

const POLLS: usize = 64;

type Flags = [bool; MOUSE_LEN];

/// Poll `i`'s pressed and update arrays: a few presses and releases.
fn drain(i: usize) -> (Flags, Flags) {
    let mut pressed = [false; MOUSE_LEN];
    let mut update = [false; MOUSE_LEN];
    for j in 0..3 {
        let slot = (i * 7 + j * 11) % (MOUSE_LEN - 1);
        update[slot] = true;
        pressed[slot] = (i + j).is_multiple_of(2);
    }
    (pressed, update)
}

type Keysym = unsafe extern "C" fn(c_int) -> c_int;
type Answer = unsafe extern "C" fn(c_int, *const bool, *const bool) -> bool;
type Commit = unsafe extern "C" fn(*const bool, *const bool, *mut bool);

fn run(keysym: Keysym, answer: Answer, commit: Commit) -> u32 {
    let mut acc = 0_u32;
    for key in 0..c_int::try_from(OBS_KEY_LAST_VALUE).unwrap() {
        // SAFETY: a plain value.
        let sym = unsafe { keysym(black_box(key)) };
        acc = acc.wrapping_mul(31).wrapping_add(sym as u32);
    }
    let mut held = [false; MOUSE_LEN];
    for i in 0..POLLS {
        let (pressed, update) = black_box(drain(i));
        for key in OBS_KEY_MOUSE1..=OBS_KEY_MOUSE29 {
            // SAFETY: both arrays hold MOUSE_LEN flags.
            let down = unsafe { answer(key, pressed.as_ptr(), held.as_ptr()) };
            acc = acc.wrapping_mul(31).wrapping_add(u32::from(down));
        }
        // SAFETY: three arrays of MOUSE_LEN flags; `held` is the bench's.
        unsafe { commit(update.as_ptr(), pressed.as_ptr(), held.as_mut_ptr()) };
    }
    acc
}

pub fn benchmarks(criterion: &mut Criterion) {
    pair(
        criterion,
        "nix_x11_input/fill_and_poll/64",
        || {
            run(
                obs_nix_x11_input_keysym,
                obs_nix_x11_input_mouse_pressed,
                obs_nix_x11_input_commit,
            )
        },
        || {
            run(
                oracle_obs_nix_x11_input_keysym,
                oracle_obs_nix_x11_input_mouse_pressed,
                oracle_obs_nix_x11_input_commit,
            )
        },
    );
}
