//! obs-hotkey: the X11 key map lookups (`libobs/obs-nix-x11-keys.c.h`).
//! The hotkey thread asks every 25 ms whether each bound key is down
//! (`key_pressed`), and `obs_key_from_virtual_key` searches the keyboard map
//! for a keysym. On a US-like map (keycodes 8 to 255, four symbols each,
//! one key per keycode), each side answers 64 pressed polls and 64 virtual
//! key searches, folded into a checksum both sides must match. The keycode
//! lists are darrays over buffers the bench owns; nothing is allocated with
//! `bmalloc`.

use super::pair;

use criterion::Criterion;
use obs_c_oracle::nix_x11_keys::{
    oracle_obs_nix_x11_keys_from_virtual_key, oracle_obs_nix_x11_keys_pressed,
};
use obs_hotkey::ffi::nix_x11_keys::{
    keycode_list, obs_nix_x11_keys_from_virtual_key, obs_nix_x11_keys_pressed,
};
use obs_hotkey::nix_x11_keys::{KEYMAP_LEN, OBS_KEY_LAST_VALUE};
use obs_util::ffi::darray::darray;
use std::ffi::{c_int, c_void};
use std::hint::black_box;

const PER: usize = 4;
const MIN: u8 = 8;
const CODES: usize = 248;
const QUERIES: usize = 64;

/// The fixture: the map, one keycode per key from key 1 on, and a bitmap
/// with every third keycode down.
struct Keyboard {
    map: Vec<u32>,
    bufs: Vec<Vec<u8>>,
    lists: Vec<darray>,
    bitmap: [u8; KEYMAP_LEN],
}

fn keyboard() -> Keyboard {
    let mut map = Vec::with_capacity(CODES * PER);
    for code in 0..CODES {
        let base = 0x20 + u32::try_from(code).unwrap();
        map.extend([base, base + 0x1000, base, base + 0x1000]);
    }
    let mut bufs = vec![Vec::new(); OBS_KEY_LAST_VALUE];
    for (code, buf) in bufs.iter_mut().skip(1).take(CODES).enumerate() {
        buf.push(MIN.wrapping_add(u8::try_from(code).unwrap()));
    }
    let lists = bufs
        .iter_mut()
        .map(|b| darray {
            array: if b.is_empty() {
                std::ptr::null_mut()
            } else {
                b.as_mut_ptr().cast()
            },
            num: b.len(),
            capacity: b.capacity(),
        })
        .collect();
    let mut bitmap = [0u8; KEYMAP_LEN];
    for code in (0..256usize).step_by(3) {
        bitmap[code / 8] |= 1 << (code % 8);
    }
    Keyboard {
        map,
        bufs,
        lists,
        bitmap,
    }
}

/// Query `i`'s key (one of the bound keys) and keysym (shifted and not).
fn query(i: usize) -> (c_int, c_int) {
    let key = 1 + (i * 37) % CODES;
    let sym = 0x20 + (i * 53) % CODES + if i.is_multiple_of(2) { 0 } else { 0x1000 };
    (
        c_int::try_from(key).unwrap(),
        c_int::try_from(sym).unwrap(),
    )
}

fn rust_queries(k: &Keyboard) -> u32 {
    let lists: *const keycode_list = k.lists.as_ptr().cast();
    let num = c_int::try_from(k.map.len()).unwrap();
    let mut acc = 0_u32;
    for i in 0..QUERIES {
        let (key, sym) = black_box(query(i));
        let at = usize::try_from(key).unwrap();
        // SAFETY: a 32-byte bitmap, the key's own list, `num` keysyms and
        // OBS_KEY_LAST_VALUE lists, all alive for the call.
        let (down, found) = unsafe {
            (
                obs_nix_x11_keys_pressed(k.bitmap.as_ptr(), key, lists.add(at), &133, &134),
                obs_nix_x11_keys_from_virtual_key(sym, k.map.as_ptr(), 4, &num, &MIN, lists),
            )
        };
        acc = acc.wrapping_mul(31).wrapping_add(u32::from(down));
        acc = acc.wrapping_mul(31).wrapping_add(found as u32);
    }
    acc
}

fn c_queries(k: &Keyboard) -> u32 {
    let lists: *const c_void = k.lists.as_ptr().cast();
    let num = c_int::try_from(k.map.len()).unwrap();
    let mut acc = 0_u32;
    for i in 0..QUERIES {
        let (key, sym) = black_box(query(i));
        let at = usize::try_from(key).unwrap();
        // SAFETY: as rust_queries; the oracle keeps no state.
        let (down, found) = unsafe {
            (
                oracle_obs_nix_x11_keys_pressed(
                    k.bitmap.as_ptr(),
                    key,
                    k.lists[at..].as_ptr().cast(),
                    &133,
                    &134,
                ),
                oracle_obs_nix_x11_keys_from_virtual_key(sym, k.map.as_ptr(), 4, &num, &MIN, lists),
            )
        };
        acc = acc.wrapping_mul(31).wrapping_add(u32::from(down));
        acc = acc.wrapping_mul(31).wrapping_add(found as u32);
    }
    acc
}

pub fn benchmarks(criterion: &mut Criterion) {
    let k = keyboard();
    assert_eq!(k.bufs.len(), OBS_KEY_LAST_VALUE);
    pair(
        criterion,
        "nix_x11_keys/poll_and_search/64",
        || rust_queries(&k),
        || c_queries(&k),
    );
}
