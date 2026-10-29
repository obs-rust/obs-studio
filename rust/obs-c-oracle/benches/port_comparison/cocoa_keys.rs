//! obs-hotkey: the macOS key map (`libobs/obs-cocoa-keys.c.h`). The event
//! tap's callback runs on every key down, key up and modifier change: it
//! reads the four modifier flags and, for a key, searches `virtual_keys` for
//! its code (`obs_key_from_virtual_key`). On a macOS-like table (the first
//! 128 keys valid with the 7-bit codes, the rest none), each side handles 64
//! key events, folded into a checksum both sides must match. The table is
//! the bench's own; nothing is allocated with `bmalloc`.

use super::pair;

use criterion::Criterion;
use obs_c_oracle::cocoa_keys::{
    oracle_obs_cocoa_keys_from_virtual_key, oracle_obs_cocoa_keys_modifier,
};
use obs_hotkey::cocoa_keys::{
    KeyCode, OBS_KEY_ALT, OBS_KEY_CONTROL, OBS_KEY_LAST_VALUE, OBS_KEY_META, OBS_KEY_SHIFT,
};
use obs_hotkey::ffi::cocoa_keys::{obs_cocoa_keys_from_virtual_key, obs_cocoa_keys_modifier};
use std::ffi::c_int;
use std::hint::black_box;

const EVENTS: usize = 64;
const MODIFIERS: [c_int; 4] = [OBS_KEY_SHIFT, OBS_KEY_ALT, OBS_KEY_META, OBS_KEY_CONTROL];

fn table() -> Vec<KeyCode> {
    (0..OBS_KEY_LAST_VALUE)
        .map(|key| KeyCode {
            code: c_int::try_from(key % 0x80).unwrap(),
            is_valid: key < 0x80,
        })
        .collect()
}

/// Event `i`'s flags (some modifiers down) and key code.
fn event(i: usize) -> (u64, c_int) {
    let flags = u64::try_from(i % 16).unwrap() << 17;
    (flags, c_int::try_from((i * 37) % 0x80).unwrap())
}

fn rust_events(table: &[KeyCode]) -> u32 {
    let mut acc = 0_u32;
    for i in 0..EVENTS {
        let (flags, code) = black_box(event(i));
        for key in MODIFIERS {
            acc = acc
                .wrapping_mul(31)
                .wrapping_add(u32::from(obs_cocoa_keys_modifier(flags, key)));
        }
        // SAFETY: OBS_KEY_LAST_VALUE entries, alive for the call.
        let key = unsafe { obs_cocoa_keys_from_virtual_key(table.as_ptr(), code) };
        acc = acc.wrapping_mul(31).wrapping_add(key.unsigned_abs());
    }
    acc
}

fn c_events(table: &[KeyCode]) -> u32 {
    let mut acc = 0_u32;
    for i in 0..EVENTS {
        let (flags, code) = black_box(event(i));
        for key in MODIFIERS {
            // SAFETY: the oracle reads only its arguments.
            let down = unsafe { oracle_obs_cocoa_keys_modifier(flags, key) };
            acc = acc.wrapping_mul(31).wrapping_add(u32::from(down));
        }
        // SAFETY: as rust_events; the oracle keeps no state.
        let key = unsafe { oracle_obs_cocoa_keys_from_virtual_key(table.as_ptr().cast(), code) };
        acc = acc.wrapping_mul(31).wrapping_add(key.unsigned_abs());
    }
    acc
}

pub fn benchmarks(criterion: &mut Criterion) {
    let table = table();
    pair(
        criterion,
        "cocoa_keys/key_events/64",
        || rust_events(&table),
        || c_events(&table),
    );
}
