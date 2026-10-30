//! obs-hotkey: the Wayland keysym tables and keymap rules
//! (`libobs/obs-nix-wayland-keys.c.h`). Each keymap the compositor sends and
//! each layout change rebuilds the keycode and keysym maps, one row per
//! keycode (`load_keymap_data`), and every focused key event looks its key
//! up (`set_focused_key`). On a US-like keymap (keycodes 8 to 255, a
//! lowercase and an uppercase keysym per keycode), each side loads the whole
//! keymap and answers 64 presses and their releases, folded into a checksum
//! both sides must match. The arrays are the bench's own; nothing is
//! allocated with `bmalloc`.

use super::pair;

use criterion::Criterion;
use obs_c_oracle::nix_wayland_keys::{
    oracle_obs_nix_wayland_keys_focused, oracle_obs_nix_wayland_keys_row,
};
use obs_hotkey::ffi::nix_wayland_keys::{obs_nix_wayland_keys_focused, obs_nix_wayland_keys_row};
use obs_hotkey::nix_wayland_keys::{MAX_KEYCODES, MAX_SHIFT_LEVELS, OBS_KEY_LAST_VALUE, to_keysym};
use std::ffi::{c_int, c_void};
use std::hint::black_box;

const FIRST: u32 = 8;
const EVENTS: u32 = 64;

/// The keymap: each keycode's level 0 and 1 keysyms (the keysyms of the
/// keys the to-table names, in key order, lowercased on level 0).
fn keymap() -> Vec<[u32; 2]> {
    let syms: Vec<u32> = (0..i32::try_from(OBS_KEY_LAST_VALUE).unwrap())
        .map(to_keysym)
        .filter(|&s| s != 0)
        .collect();
    (FIRST..u32::try_from(MAX_KEYCODES).unwrap())
        .map(|code| {
            let sym = syms[usize::try_from(code).unwrap() % syms.len()];
            let lower = if (0x41..=0x5a).contains(&sym) { sym + 0x20 } else { sym };
            [lower, sym]
        })
        .collect()
}

/// `xkb_keymap_key_get_syms_by_level()` on the bench keymap: levels 0 and 1.
unsafe extern "C" fn level_syms(data: *mut c_void, level: u32, syms: *mut *const u32) -> c_int {
    // SAFETY: `data` is the row the caller passed, alive for the call.
    let row = unsafe { &*data.cast::<[u32; 2]>() };
    let Some(sym) = usize::try_from(level).ok().and_then(|l| row.get(l)) else {
        return 0;
    };
    // SAFETY: the callee passes a writable pointer.
    unsafe { syms.write(sym) };
    1
}

type RowFn = unsafe extern "C" fn(
    u32,
    Option<unsafe extern "C" fn(*mut c_void, u32, *mut *const u32) -> c_int>,
    *mut c_void,
    *mut u32,
    *mut [u32; MAX_KEYCODES],
);
type FocusedFn = unsafe extern "C" fn(u32, bool, *const u32, *mut c_int, *mut c_int) -> bool;

fn run(keymap: &[[u32; 2]], row: RowFn, focused: FocusedFn) -> u32 {
    let mut obs_to_key = vec![0u32; OBS_KEY_LAST_VALUE];
    let mut key_to_sym = vec![[0u32; MAX_KEYCODES]; MAX_SHIFT_LEVELS];
    let mut pressed_as: Vec<c_int> = vec![0; MAX_KEYCODES];
    for (code, syms) in (FIRST..).zip(keymap) {
        let mut syms = black_box(*syms);
        // SAFETY: the arrays have the sizes the C reads; the row outlives
        // the call.
        unsafe {
            row(
                code,
                Some(level_syms),
                (&raw mut syms).cast(),
                obs_to_key.as_mut_ptr(),
                key_to_sym.as_mut_ptr(),
            );
        }
    }
    let mut acc = obs_to_key.iter().fold(0_u32, |a, &v| a.wrapping_mul(31).wrapping_add(v));
    for pressed in [true, false] {
        for i in 0..EVENTS {
            let scancode = black_box(i * 37 % 248);
            let mut key: c_int = 0;
            // SAFETY: as above.
            let hit = unsafe {
                focused(scancode, pressed, key_to_sym[0].as_ptr(), pressed_as.as_mut_ptr(), &raw mut key)
            };
            acc = acc.wrapping_mul(31).wrapping_add(u32::from(hit));
            acc = acc.wrapping_mul(31).wrapping_add(u32::from_ne_bytes(key.to_ne_bytes()));
        }
    }
    acc
}

pub fn benchmarks(criterion: &mut Criterion) {
    let keymap = keymap();
    pair(
        criterion,
        "nix_wayland_keys/keymap_and_keys/248",
        || run(&keymap, obs_nix_wayland_keys_row, obs_nix_wayland_keys_focused),
        || run(&keymap, oracle_obs_nix_wayland_keys_row, oracle_obs_nix_wayland_keys_focused),
    );
}
